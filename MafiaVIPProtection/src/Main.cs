using System;
using System.Windows.Forms;
using GTA;

namespace MafiaVIP
{
    /// <summary>
    /// Mafia VIP Protection - ana script sinifi.
    ///
    /// Performans notu: Tick her karede calisir ama agir islemler
    /// zamanlayici ile bolunmustur (tehdit taramasi 750 ms, koruma 400 ms,
    /// konvoy 900 ms, hava destegi 1200 ms, takviye 1000 ms).
    /// Menu isleme LemonUI gerektirdigi icin her karede yapilir.
    /// </summary>
    public class MafiaVipProtection : Script
    {
        // Guncelleme araliklari (ms)
        private const int ThreatInterval = 750;
        private const int GuardInterval = 400;
        private const int ConvoyInterval = 900;
        private const int AirInterval = 1200;
        private const int BackupInterval = 1000;
        private const int TransferInterval = 500;

        private Config _cfg;
        private ThreatScanner _scanner;
        private CloseProtection _guards;
        private ConvoyManager _convoy;
        private AirSupport _air;
        private BackupManager _backup;
        private VipTransfer _transfer;
        private MenuSystem _menu;

        private bool _initialized;
        private bool _autoStartDone;

        private int _nextThreat;
        private int _nextGuard;
        private int _nextConvoy;
        private int _nextAir;
        private int _nextBackup;
        private int _nextTransfer;

        public MafiaVipProtection()
        {
            Interval = 0;                 // menu icin her kare
            Tick += OnTick;
            KeyDown += OnKeyDown;
            Aborted += OnAborted;
        }

        // ==================================================================
        // Kurulum
        // ==================================================================
        private void Initialize()
        {
            _cfg = Config.Load();
            Utils.NotificationsEnabled = _cfg.ShowNotifications;

            Relationships.Setup(_cfg.RelationshipGroupName);

            _scanner = new ThreatScanner(_cfg);
            _guards = new CloseProtection(_cfg, _scanner);
            _convoy = new ConvoyManager(_cfg, _scanner);
            _air = new AirSupport(_cfg, _scanner);
            _backup = new BackupManager(_cfg);
            _transfer = new VipTransfer(_cfg);

            // Yakin koruma, oyuncunun aracinda yer kalmazsa konvoy araclarini kullanir.
            _guards.SupportVehicleProvider = _convoy.FindVehicleWithFreeSeat;
            _convoy.Behavior = _guards.Behavior;

            _menu = new MenuSystem(_cfg, _guards, _convoy, _air, _backup, _transfer, ClearAll, ReloadConfig);

            _initialized = true;
            Logger.Info("Mafia VIP Protection yuklendi.");
            Utils.Notify("Mafia VIP Protection hazir. Menu: " + _cfg.MenuKey);
        }

        private void ReloadConfig()
        {
            Utils.Notify("Ayarlar yeniden yukleniyor...");
            ClearAll();

            _initialized = false;
            _autoStartDone = true;        // yeniden yuklemede otomatik baslatma yapma
            Initialize();
        }

        private void ClearAll()
        {
            try
            {
                if (_guards != null) _guards.CleanupAll();
                if (_convoy != null) _convoy.Dismiss(true);
                if (_air != null) _air.CleanupAll();
                if (_backup != null) _backup.CleanupAll();
                if (_transfer != null) _transfer.CleanupAll();
                if (_scanner != null) _scanner.Clear();

                Utils.Notify("Tum ekipler temizlendi.");
            }
            catch (Exception ex)
            {
                Logger.Error("ClearAll hatasi", ex);
            }
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        private void OnTick(object sender, EventArgs e)
        {
            try
            {
                if (Game.IsLoading) return;

                if (!_initialized)
                {
                    Initialize();
                    return;
                }

                _menu.Process();

                Ped player = Game.Player.Character;
                if (!Utils.Valid(player)) return;

                // Ayarda aciksa ilk uygun anda korumalari cagir.
                if (!_autoStartDone)
                {
                    _autoStartDone = true;
                    if (_cfg.AutoStartOnLoad && player.IsAlive) _guards.Deploy();
                }

                int now = Game.GameTime;

                if (now >= _nextThreat)
                {
                    _nextThreat = now + ThreatInterval;
                    _scanner.Update(player, _guards.Peds, _guards.Behavior);
                }

                if (now >= _nextGuard)
                {
                    _nextGuard = now + GuardInterval;
                    _guards.Update();
                }

                if (now >= _nextConvoy)
                {
                    _nextConvoy = now + ConvoyInterval;
                    _convoy.Update();
                }

                if (now >= _nextAir)
                {
                    _nextAir = now + AirInterval;
                    _air.Update();
                }

                if (now >= _nextBackup)
                {
                    _nextBackup = now + BackupInterval;
                    _backup.Update();
                }

                if (now >= _nextTransfer)
                {
                    _nextTransfer = now + TransferInterval;
                    _transfer.Update();
                }
            }
            catch (Exception ex)
            {
                Logger.Error("OnTick hatasi", ex);
            }
        }

        // ==================================================================
        // Tuslar
        // ==================================================================
        /// <summary>
        /// Tek tus: menu. Baska modlarla tus catismasi yasanmasin diye tum
        /// islemler (koruma, konvoy, hava destegi, takviye, VIP transfer,
        /// formasyon, temizle...) yalniz F7 menusu uzerinden yapilir.
        /// </summary>
        private void OnKeyDown(object sender, KeyEventArgs e)
        {
            try
            {
                if (!_initialized || _cfg == null) return;

                if (e.KeyCode == _cfg.MenuKey)
                    _menu.Toggle();
            }
            catch (Exception ex)
            {
                Logger.Error("OnKeyDown hatasi", ex);
            }
        }

        // ==================================================================
        // Kapanis
        // ==================================================================
        private void OnAborted(object sender, EventArgs e)
        {
            try
            {
                if (_guards != null) _guards.CleanupAll();
                if (_convoy != null) _convoy.Dismiss(true);
                if (_air != null) _air.CleanupAll();
                if (_backup != null) _backup.CleanupAll();
                if (_transfer != null) _transfer.CleanupAll();
                Logger.Info("Script kapatildi, tum varliklar temizlendi.");
            }
            catch (Exception ex)
            {
                Logger.Error("OnAborted hatasi", ex);
            }
        }
    }
}
