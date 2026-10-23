package com.crashbit.pvpccheap4.data.billing

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.AcknowledgePurchaseParams
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingClientStateListener
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.BillingResult
import com.android.billingclient.api.PendingPurchasesParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesUpdatedListener
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.QueryPurchasesParams
import com.android.billingclient.api.acknowledgePurchase
import com.android.billingclient.api.queryProductDetails
import com.android.billingclient.api.queryPurchasesAsync
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.api.ApiService
import com.crashbit.pvpccheap4.data.model.GooglePurchaseRequest
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.coroutines.resume

/** A way to pay for the paid plan: "mensual" or "anual", with its price in the user's currency */
data class Plan(val basePlanId: String, val price: String, val offerToken: String)

data class BillingState(
    /** Null while loading; empty when the store has no plans (not installed from Google Play) */
    val plans: List<Plan>? = null,
    val isWorking: Boolean = false,
    /** A purchase reached the server: the account must be read again */
    val purchased: Boolean = false,
    val message: String? = null
)

/**
 * Google Play purchases of the paid plan: one subscription ("premium") with two
 * base plans. The server checks every purchase with Google before unlocking it.
 */
@Singleton
class BillingManager @Inject constructor(
    @ApplicationContext private val context: Context,
    private val apiService: ApiService
) : PurchasesUpdatedListener {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val _state = MutableStateFlow(BillingState())
    val state: StateFlow<BillingState> = _state.asStateFlow()

    private var productDetails: ProductDetails? = null

    private val client = BillingClient.newBuilder(context)
        .setListener(this)
        .enablePendingPurchases(PendingPurchasesParams.newBuilder().enableOneTimeProducts().build())
        // The library reconnects by itself when Google Play drops the service
        .enableAutoServiceReconnection()
        .build()

    private suspend fun connect(): Boolean {
        if (client.isReady) return true
        return suspendCancellableCoroutine { cont ->
            client.startConnection(object : BillingClientStateListener {
                override fun onBillingSetupFinished(result: BillingResult) {
                    if (cont.isActive) cont.resume(result.responseCode == BillingClient.BillingResponseCode.OK)
                }

                override fun onBillingServiceDisconnected() {
                    if (cont.isActive) cont.resume(false)
                }
            })
        }
    }

    /** Reads the plans and their prices from the store */
    fun loadPlans() {
        scope.launch {
            if (!connect()) {
                _state.value = _state.value.copy(plans = emptyList())
                return@launch
            }
            val params = QueryProductDetailsParams.newBuilder()
                .setProductList(
                    listOf(
                        QueryProductDetailsParams.Product.newBuilder()
                            .setProductId(PRODUCT_ID)
                            .setProductType(BillingClient.ProductType.SUBS)
                            .build()
                    )
                )
                .build()
            val result = client.queryProductDetails(params)
            val details = result.productDetailsList?.firstOrNull()
            productDetails = details
            val plans = details?.subscriptionOfferDetails.orEmpty()
                // The base plans themselves, not promotional offers
                .filter { it.offerId == null }
                .mapNotNull { offer ->
                    val price = offer.pricingPhases.pricingPhaseList.lastOrNull()?.formattedPrice ?: return@mapNotNull null
                    Plan(offer.basePlanId, price, offer.offerToken)
                }
                .sortedBy { if (it.basePlanId == MONTHLY) 0 else 1 }
            _state.value = _state.value.copy(plans = plans)
        }
    }

    /** Opens Google Play's payment sheet; the result arrives in onPurchasesUpdated */
    fun buy(activity: Activity, plan: Plan, userId: Int) {
        val details = productDetails ?: return
        val params = BillingFlowParams.newBuilder()
            .setProductDetailsParamsList(
                listOf(
                    BillingFlowParams.ProductDetailsParams.newBuilder()
                        .setProductDetails(details)
                        .setOfferToken(plan.offerToken)
                        .build()
                )
            )
            // The server checks that the purchase is this account's
            .setObfuscatedAccountId("u$userId")
            .build()
        client.launchBillingFlow(activity, params)
    }

    override fun onPurchasesUpdated(result: BillingResult, purchases: MutableList<Purchase>?) {
        when (result.responseCode) {
            BillingClient.BillingResponseCode.OK -> scope.launch { purchases.orEmpty().forEach { send(it) } }
            BillingClient.BillingResponseCode.USER_CANCELED -> {}
            // Google Play blocked on this phone (e.g. a kids mode)
            BillingClient.BillingResponseCode.BILLING_UNAVAILABLE ->
                _state.value = _state.value.copy(message = context.getString(R.string.account_billing_unavailable))
            else -> _state.value = _state.value.copy(message = context.getString(R.string.account_billing_failed))
        }
    }

    /** Sends the purchases the store has for this phone again (after reinstalling, or if a send failed) */
    fun restore() {
        scope.launch {
            if (!connect()) {
                _state.value = _state.value.copy(message = context.getString(R.string.account_billing_unavailable))
                return@launch
            }
            val result = client.queryPurchasesAsync(
                QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.SUBS).build()
            )
            // A suspended subscription (payment declined) gives no access
            val purchases = result.purchasesList.filter { it.purchaseState == Purchase.PurchaseState.PURCHASED && !it.isSuspended }
            if (purchases.isEmpty()) {
                _state.value = _state.value.copy(message = context.getString(R.string.account_billing_no_purchases))
            }
            purchases.forEach { send(it) }
        }
    }

    private suspend fun send(purchase: Purchase) {
        if (purchase.purchaseState != Purchase.PurchaseState.PURCHASED) {
            if (purchase.purchaseState == Purchase.PurchaseState.PENDING) {
                _state.value = _state.value.copy(message = context.getString(R.string.account_billing_pending))
            }
            return
        }
        _state.value = _state.value.copy(isWorking = true)
        val ok = try {
            apiService.googlePurchase(GooglePurchaseRequest(purchase.purchaseToken)).isSuccessful
        } catch (e: Exception) {
            false
        }
        if (ok && !purchase.isAcknowledged) {
            client.acknowledgePurchase(
                AcknowledgePurchaseParams.newBuilder().setPurchaseToken(purchase.purchaseToken).build()
            )
        }
        _state.value = _state.value.copy(
            isWorking = false,
            purchased = ok,
            message = if (ok) context.getString(R.string.account_billing_success)
            else context.getString(R.string.account_billing_activation_failed)
        )
    }

    fun consumeMessage() {
        _state.value = _state.value.copy(message = null, purchased = false)
    }

    companion object {
        const val PRODUCT_ID = "premium"
        const val MONTHLY = "mensual"
        const val YEARLY = "anual"
    }
}
