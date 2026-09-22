# Add project specific ProGuard rules here.
# By default, the flags in this file are appended to flags specified
# in /usr/local/Cellar/android-sdk/24.3.3/tools/proguard/proguard-android.txt
# You can edit the include path and order by changing the proguardFiles
# directive in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# Add any project specific keep options here:

# Reanimated 4 and Worklets keep class/method names so the worklet runtime can
# look them up from a string. Minification is off today; these are for when it
# is turned on.
-keep class com.swmansion.worklets.** { *; }
-keep class com.swmansion.reanimated.** { *; }
-keep class com.facebook.react.turbomodule.** { *; }

# Gesture Handler's touch target helpers are looked up the same way.
-keep class com.swmansion.gesturehandler.** { *; }
