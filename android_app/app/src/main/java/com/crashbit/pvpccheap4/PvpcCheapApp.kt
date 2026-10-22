package com.crashbit.pvpccheap4

import android.app.Application
import com.crashbit.pvpccheap4.data.repository.ResultMessages
import dagger.hilt.android.HiltAndroidApp

@HiltAndroidApp
class PvpcCheapApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Network errors in the phone's language, whichever repository fails first
        ResultMessages.init(this)
    }
}
