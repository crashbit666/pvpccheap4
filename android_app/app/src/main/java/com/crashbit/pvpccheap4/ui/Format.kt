package com.crashbit.pvpccheap4.ui

import java.util.Locale

private val catalan = Locale.forLanguageTag("ca")

/** 0.1234 → "0,1234 €/kWh", with the decimal comma used in Catalan. */
fun formatPrice(price: Double): String = String.format(catalan, "%.4f €/kWh", price)
