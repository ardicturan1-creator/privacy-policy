plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
    alias(libs.plugins.kotlin.serialization)
}

android { namespace = "com.ultraguard.core.engine" }

dependencies {
    api(projects.core.model)
    api(projects.core.ai)
    implementation(projects.core.common)
    implementation(projects.core.database)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.hilt.work)
}
