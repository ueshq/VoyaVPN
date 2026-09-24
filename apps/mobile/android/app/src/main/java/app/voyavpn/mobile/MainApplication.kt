package app.voyavpn.mobile

import android.app.Application
import com.facebook.react.PackageList
import com.facebook.react.ReactApplication
import com.facebook.react.ReactHost
import com.facebook.react.ReactNativeApplicationEntryPoint.loadReactNative
import com.facebook.react.defaults.DefaultReactHost.getDefaultReactHost

import app.voyavpn.mobile.host.VoyaNativePackage

class MainApplication : Application(), ReactApplication {

  override val reactHost: ReactHost by lazy {
    getDefaultReactHost(
      context = applicationContext,
      packageList =
        PackageList(this).packages.apply {
          // Autolinking only sees packages in node_modules; this one lives in
          // the app, because it fronts the Rust host built beside it.
          add(VoyaNativePackage())
        },
    )
  }

  override fun onCreate() {
    super.onCreate()
    go.Seq.setContext(this)
    loadReactNative(this)
  }
}
