package app.voyavpn.mobile.host

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.uimanager.ViewManager

/**
 * Registers [VoyaNativeModule], which autolinking cannot do: it is a module
 * inside the app rather than a package in `node_modules`, so `MainApplication`
 * adds it to the package list by hand.
 */
class VoyaNativePackage : ReactPackage {
    override fun createNativeModules(context: ReactApplicationContext): List<NativeModule> =
        listOf(VoyaNativeModule(context))

    override fun createViewManagers(context: ReactApplicationContext): List<ViewManager<*, *>> =
        emptyList()
}
