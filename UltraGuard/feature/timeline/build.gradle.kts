plugins {
    id("ultraguard.android.feature")
}

android { namespace = "com.ultraguard.feature.timeline" }

dependencies {
    implementation(projects.core.database)
}
