plugins {
    id("ultraguard.android.library")
    id("ultraguard.android.hilt")
}

android { namespace = "com.ultraguard.module.deepscan" }

dependencies {
    api(projects.core.model)
    implementation(projects.core.common)
    implementation(projects.core.engine)
}
