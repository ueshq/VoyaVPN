package app.voyavpn.mobile.host

import com.facebook.react.BaseReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.model.ReactModuleInfo
import com.facebook.react.module.model.ReactModuleInfoProvider

/**
 * Registers [VoyaNativeModule] and [VoyaDeviceActions], which autolinking
 * cannot do: they are modules inside the app rather than a package in
 * `node_modules`, so `MainApplication` adds this to the package list by hand.
 *
 * Each module is created when JavaScript first asks for it.
 */
class VoyaNativePackage : BaseReactPackage() {
    override fun getModule(name: String, reactContext: ReactApplicationContext): NativeModule? =
        when (name) {
            VoyaNativeModule.NAME -> VoyaNativeModule(reactContext)
            VoyaDeviceActions.NAME -> VoyaDeviceActions(reactContext)
            else -> null
        }

    override fun getReactModuleInfoProvider() = ReactModuleInfoProvider {
        mapOf(
            VoyaNativeModule.NAME to info(VoyaNativeModule.NAME, VoyaNativeModule::class.java),
            VoyaDeviceActions.NAME to info(VoyaDeviceActions.NAME, VoyaDeviceActions::class.java),
        )
    }

    private fun info(name: String, module: Class<out NativeModule>) = ReactModuleInfo(
        name = name,
        className = module.name,
        canOverrideExistingModule = false,
        needsEagerInit = false,
        isCxxModule = false,
        isTurboModule = false,
    )
}
