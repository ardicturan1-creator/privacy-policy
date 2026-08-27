plugins {
    id("ultraguard.android.feature")
}

android { namespace = "com.ultraguard.feature.assistant" }

dependencies {
    implementation(projects.core.database)
}
