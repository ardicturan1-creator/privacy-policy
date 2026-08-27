plugins {
    `kotlin-dsl`
}

group = "com.ultraguard.buildlogic"

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    compileOnly(libs.android.gradlePlugin)
    compileOnly(libs.kotlin.gradlePlugin)
    compileOnly(libs.compose.gradlePlugin)
    compileOnly(libs.ksp.gradlePlugin)
}

gradlePlugin {
    plugins {
        register("androidApplication") {
            id = "ultraguard.android.application"
            implementationClass = "com.ultraguard.buildlogic.AndroidApplicationConventionPlugin"
        }
        register("androidLibrary") {
            id = "ultraguard.android.library"
            implementationClass = "com.ultraguard.buildlogic.AndroidLibraryConventionPlugin"
        }
        register("androidCompose") {
            id = "ultraguard.android.compose"
            implementationClass = "com.ultraguard.buildlogic.AndroidComposeConventionPlugin"
        }
        register("androidHilt") {
            id = "ultraguard.android.hilt"
            implementationClass = "com.ultraguard.buildlogic.AndroidHiltConventionPlugin"
        }
        register("jvmLibrary") {
            id = "ultraguard.jvm.library"
            implementationClass = "com.ultraguard.buildlogic.JvmLibraryConventionPlugin"
        }
        register("androidFeature") {
            id = "ultraguard.android.feature"
            implementationClass = "com.ultraguard.buildlogic.AndroidFeatureConventionPlugin"
        }
    }
}
