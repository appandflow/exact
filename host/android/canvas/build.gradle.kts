plugins { id("com.android.library") }
// An app's build gives the reader a build directory of its own (`-PcanvasBuildDir`, agent-android.mjs).
providers.gradleProperty("canvasBuildDir").orNull?.let { layout.buildDirectory = file(it) }
android {
    namespace = "dev.exact.canvas"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { minSdk = 29; consumerProguardFiles("proguard-rules.pro") }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}

// Exercise the alarm receivers and service without linking the package into the reader.
androidComponents.onVariants { variant ->
    variant.hostTests.values.forEach { test ->
        test.sources.kotlin?.addStaticSourceDirectory("../../../packages/alarms/android")
        test.sources.kotlin?.addStaticSourceDirectory("../../../packages/alarms/test")
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    testImplementation(libs.robolectric)
}
