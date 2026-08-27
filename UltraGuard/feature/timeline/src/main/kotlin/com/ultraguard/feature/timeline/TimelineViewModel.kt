package com.ultraguard.feature.timeline

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.EventDao
import com.ultraguard.core.model.EventDomain
import com.ultraguard.core.model.EventType
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn

/**
 * Zaman cizelgesi -- "hangi uygulama ne zaman ne yapti".
 *
 * Bu ekran urunun seffaflik vaadinin somut karsiligidir: kullanici, bizim
 * gordugumuz her seyi gorebilir. Gizli bir telemetri katmani yoktur; burada
 * gosterilmeyen bir olay toplanmamis demektir.
 */
@HiltViewModel
class TimelineViewModel @Inject constructor(
    private val eventDao: EventDao,
    private val clock: Clock,
) : ViewModel() {

    private val filter = MutableStateFlow(TimelineFilter())

    @OptIn(ExperimentalCoroutinesApi::class)
    val uiState: StateFlow<TimelineUiState> = combine(
        eventDao.recentStream(clock.nowMillis() - WINDOW_MILLIS, limit = PAGE_SIZE),
        filter,
    ) { events, activeFilter ->
        TimelineUiState(
            filter = activeFilter,
            entries = events
                .filter { activeFilter.matches(it.type, it.packageName) }
                .map { entity ->
                    TimelineEntry(
                        eventId = entity.id,
                        timestampMillis = entity.timestampMillis,
                        packageName = entity.packageName ?: SYSTEM_SUBJECT,
                        type = entity.type,
                    )
                },
        )
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.WhileSubscribed(STOP_TIMEOUT_MILLIS),
        initialValue = TimelineUiState(),
    )

    fun setDomainFilter(domain: EventDomain?) {
        filter.value = filter.value.copy(domain = domain)
    }

    fun setPackageFilter(packageName: String?) {
        filter.value = filter.value.copy(packageName = packageName)
    }

    private companion object {
        const val WINDOW_MILLIS = 7 * 24 * 60 * 60 * 1000L
        const val PAGE_SIZE = 300
        const val STOP_TIMEOUT_MILLIS = 5_000L
        const val SYSTEM_SUBJECT = "Sistem"
    }
}

data class TimelineUiState(
    val filter: TimelineFilter = TimelineFilter(),
    val entries: List<TimelineEntry> = emptyList(),
)

data class TimelineFilter(
    val domain: EventDomain? = null,
    val packageName: String? = null,
) {
    fun matches(type: EventType, eventPackage: String?): Boolean {
        if (domain != null && type.domain != domain) return false
        if (packageName != null && eventPackage != packageName) return false
        return true
    }
}

data class TimelineEntry(
    val eventId: Long,
    val timestampMillis: Long,
    val packageName: String,
    val type: EventType,
)
