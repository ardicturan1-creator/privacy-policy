package com.ultraguard.core.policy

import com.google.common.truth.Truth.assertThat
import com.ultraguard.core.common.time.Clock
import com.ultraguard.core.database.dao.LedgerDao
import com.ultraguard.core.database.entity.LedgerEntryEntity
import com.ultraguard.core.model.ActionOutcome
import com.ultraguard.core.model.EnforcementAction
import com.ultraguard.core.model.ProtectionMode
import com.ultraguard.core.model.RevertActor
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Test

/**
 * Yaptirim uygulayicinin degismezleri.
 *
 * Bu testlerin varlik nedeni: bir yanlis pozitifin kullaniciya kalici zarar
 * verememesi, yalnizca kod incelemesine degil calisan bir kanita
 * dayanmalidir.
 */
class EnforcementExecutorTest {

    private val dispatcher = StandardTestDispatcher()
    private val clock = object : Clock {
        var now = 1_700_000_000_000L
        override fun nowMillis() = now
        override fun elapsedRealtimeMillis() = now
    }

    private val ledgerDao = FakeLedgerDao()
    private val network = FakeNetworkPort()
    private val overlay = FakeOverlayPort()
    private val deviceAdmin = FakeDeviceAdminPort()
    private val process = FakeProcessPort()
    private val userActions = FakeUserActionPort()

    private fun executor() = EnforcementExecutor(
        ledger = ActionLedger(ledgerDao, clock, dispatcher),
        network = network,
        overlay = overlay,
        deviceAdmin = deviceAdmin,
        process = process,
        userActions = userActions,
        ioDispatcher = dispatcher,
    )

    private fun plan(
        autonomous: List<EnforcementAction> = emptyList(),
        consent: List<EnforcementAction> = emptyList(),
        unavailable: List<EnforcementAction> = emptyList(),
    ) = EnforcementPlan(
        autonomous = autonomous,
        requiresConsent = consent,
        unavailable = unavailable,
        notifyUser = true,
        escalateMonitoring = true,
    )

    @Test
    fun `ag engelleme uygulanir ve deftere yazilir`() = runTest(dispatcher) {
        val action = EnforcementAction.SuspendNetwork("com.evil", uid = 10234)

        val report = executor().execute(plan(autonomous = listOf(action)), ProtectionMode.ACTIVE, 1L)

        assertThat(network.blocked).containsExactly(10234)
        assertThat(report.applied).containsExactly(action)
        assertThat(ledgerDao.entries).hasSize(1)
        assertThat(ledgerDao.entries.first().outcome).isEqualTo(ActionOutcome.APPLIED)
    }

    @Test
    fun `basarisiz eylem APPLIED degil FAILED olarak kaydedilir`() = runTest(dispatcher) {
        overlay.shouldSucceed = false
        val action = EnforcementAction.HideOverlays("com.evil")

        val report = executor().execute(plan(autonomous = listOf(action)), ProtectionMode.ACTIVE, 1L)

        assertThat(report.applied).isEmpty()
        assertThat(report.failed).containsExactly(action)
        assertThat(ledgerDao.entries.first().outcome).isEqualTo(ActionOutcome.FAILED)
    }

    @Test
    fun `onay bekleyen eylem uygulanmaz ama deftere yazilir`() = runTest(dispatcher) {
        val action = EnforcementAction.RequestUninstall("com.evil")

        executor().execute(plan(consent = listOf(action)), ProtectionMode.ACTIVE, 1L)

        assertThat(userActions.uninstallRequests).isEmpty()
        assertThat(ledgerDao.entries.first().outcome)
            .isEqualTo(ActionOutcome.AWAITING_USER_CONSENT)
    }

    @Test
    fun `yetki olmayan eylem sessizce atlanmaz`() = runTest(dispatcher) {
        val action = EnforcementAction.FreezeProcess("com.evil", pid = 4242)

        executor().execute(plan(unavailable = listOf(action)), ProtectionMode.ACTIVE, 1L)

        assertThat(process.frozen).isEmpty()
        assertThat(ledgerDao.entries.first().outcome)
            .isEqualTo(ActionOutcome.SKIPPED_NO_CAPABILITY)
    }

    @Test
    fun `kullanici onayi ile kaldirma akisi acilir`() = runTest(dispatcher) {
        val action = EnforcementAction.RequestUninstall("com.evil")

        val success = executor().executeWithConsent(action, ProtectionMode.ACTIVE, 1L)

        assertThat(success).isTrue()
        assertThat(userActions.uninstallRequests).containsExactly("com.evil")
    }

    @Test
    fun `geri alma sistem durumunu eski haline dondurur`() = runTest(dispatcher) {
        val action = EnforcementAction.SuspendNetwork("com.evil", uid = 10234)
        executor().execute(plan(autonomous = listOf(action)), ProtectionMode.ACTIVE, 1L)
        assertThat(network.blocked).containsExactly(10234)

        val entryId = ledgerDao.entries.first().id
        val reverted = executor().revert(entryId, RevertActor.USER)

        assertThat(reverted).isTrue()
        assertThat(network.blocked).isEmpty()
        assertThat(ledgerDao.entries.first().revertedAtMillis).isNotNull()
    }

    @Test
    fun `ayni kayit iki kez geri alinamaz`() = runTest(dispatcher) {
        val action = EnforcementAction.SuspendNetwork("com.evil", uid = 10234)
        val exec = executor()
        exec.execute(plan(autonomous = listOf(action)), ProtectionMode.ACTIVE, 1L)
        val entryId = ledgerDao.entries.first().id

        assertThat(exec.revert(entryId, RevertActor.USER)).isTrue()
        assertThat(exec.revert(entryId, RevertActor.USER)).isFalse()
    }

    @Test
    fun `defter zinciri ardisik yazimlarda saglam kalir`() = runTest(dispatcher) {
        val exec = executor()
        exec.execute(
            plan(
                autonomous = listOf(
                    EnforcementAction.SuspendNetwork("com.a", 1),
                    EnforcementAction.HideOverlays("com.b"),
                    EnforcementAction.SuspendNetwork("com.c", 3),
                ),
            ),
            ProtectionMode.ACTIVE,
            1L,
        )

        val ledger = ActionLedger(ledgerDao, clock, dispatcher)
        assertThat(ledger.verifyIntegrity()).isInstanceOf(LedgerIntegrity.Intact::class.java)
    }

    // ------------------------------------------------------------------
    // Sahteler
    // ------------------------------------------------------------------

    private class FakeNetworkPort : NetworkEnforcementPort {
        val blocked = mutableSetOf<Int>()
        override fun blockUid(uid: Int, untilMillis: Long?) { blocked += uid }
        override fun unblockUid(uid: Int) { blocked -= uid }
    }

    private class FakeOverlayPort : OverlayEnforcementPort {
        var shouldSucceed = true
        val hidden = mutableSetOf<String>()
        override fun hideOverlaysFor(packageName: String): Boolean {
            if (!shouldSucceed) return false
            hidden += packageName
            return true
        }
        override fun stopHidingOverlaysFor(packageName: String) { hidden -= packageName }
    }

    private class FakeDeviceAdminPort : DeviceAdminEnforcementPort {
        override fun suspendPackage(packageName: String) = false
        override fun unsuspendPackage(packageName: String) = false
        override fun revokePermission(packageName: String, permission: String) = false
        override fun grantPermission(packageName: String, permission: String) = false
    }

    private class FakeProcessPort : ProcessEnforcementPort {
        val frozen = mutableSetOf<Int>()
        override suspend fun freeze(pid: Int): Boolean { frozen += pid; return true }
        override suspend fun unfreeze(pid: Int): Boolean { frozen -= pid; return true }
    }

    private class FakeUserActionPort : UserActionPort {
        val uninstallRequests = mutableListOf<String>()
        val permissionScreens = mutableListOf<Pair<String, String>>()
        override fun requestUninstall(packageName: String) { uninstallRequests += packageName }
        override fun openPermissionSettings(packageName: String, permission: String) {
            permissionScreens += packageName to permission
        }
    }

    /** Salt-ekleme davranisini birebir taklit eden bellek ici defter. */
    private class FakeLedgerDao : LedgerDao {
        val entries = mutableListOf<LedgerEntryEntity>()
        private var nextId = 1L

        override suspend fun append(entry: LedgerEntryEntity): Long {
            val id = nextId++
            entries += entry.copy(id = id)
            return id
        }

        override suspend fun last(): LedgerEntryEntity? = entries.lastOrNull()

        override suspend fun byId(id: Long): LedgerEntryEntity? = entries.firstOrNull { it.id == id }

        override suspend fun allAscending(): List<LedgerEntryEntity> = entries.toList()

        override fun recentStream(limit: Int): Flow<List<LedgerEntryEntity>> =
            flowOf(entries.takeLast(limit).reversed())

        override suspend fun activeActionsFor(packageName: String): List<LedgerEntryEntity> =
            entries.filter {
                it.packageName == packageName &&
                    it.revertedAtMillis == null &&
                    it.outcome == ActionOutcome.APPLIED
            }

        override suspend fun allActiveActions(): List<LedgerEntryEntity> =
            entries.filter { it.revertedAtMillis == null && it.outcome == ActionOutcome.APPLIED }

        override suspend fun markReverted(id: Long, atMillis: Long, actor: String) {
            val index = entries.indexOfFirst { it.id == id }
            if (index >= 0) {
                entries[index] = entries[index].copy(revertedAtMillis = atMillis, revertedBy = actor)
            }
        }

        override suspend fun updateOutcome(id: Long, outcome: ActionOutcome) {
            val index = entries.indexOfFirst { it.id == id }
            if (index >= 0) entries[index] = entries[index].copy(outcome = outcome)
        }

        override suspend fun count(): Int = entries.size
    }
}
