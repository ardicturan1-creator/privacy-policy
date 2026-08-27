plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
    alias(libs.plugins.ksp)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.ultraguard.core.database"

    sourceSets.getByName("androidTest").assets.srcDir("$projectDir/schemas")
}

// Semalar surum kontrolune girer; migration testleri bunlara dayanir.
// KSP argumanlari ust duzey `ksp` blogunda tanimlanir -- `defaultConfig`
// icinde degil.
ksp {
    arg("room.schemaLocation", "$projectDir/schemas")
}

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(projects.core.security)

    implementation(libs.androidx.room.runtime)
    implementation(libs.androidx.room.ktx)
    implementation(libs.androidx.sqlite.ktx)
    implementation(libs.sqlcipher)
    implementation(libs.kotlinx.serialization.json)
    ksp(libs.androidx.room.compiler)

    androidTestImplementation(libs.androidx.room.testing)
    androidTestImplementation(libs.androidx.test.junit)
}
