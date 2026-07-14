plugins {
    id("com.android.application")
}

android {
    namespace = "com.kzahel.physbox5"
    compileSdk = 35
    ndkVersion = "27.0.12077973"

    defaultConfig {
        applicationId = "com.kzahel.physbox5"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    sourceSets["main"].jniLibs.srcDirs("../jniLibs")
}
