using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Kara yoluyla takviye birligi.
    ///
    /// Uzakta bir arac spawn edilir, oyuncuya dogru surer, yaklastiginda
    /// ekip iner ve yakin koruma ekibine katilir. Arac ve surucu sonra
    /// serbest birakilir (oyun motoru temizler).
    /// </summary>
    internal sealed class BackupManager
    {
        private readonly Config _cfg;
        private readonly List<Ped> _squad = new List<Ped>();

        private Vehicle _vehicle;
        private Ped _driver;
        private Blip _blip;
        private int _stage;          // 0 yok, 1 yolda, 2 iniyor
        private int _stageTime;
        private int _lastCallTime = -999999;
        private Func<Ped, bool> _adoptCallback;

        // Sikisma tespiti
        private Vector3 _lastCheckPosition;
        private int _lastMovedTime;

        public BackupManager(Config cfg)
        {
            _cfg = cfg;
        }

        public bool Busy { get { return _stage != 0; } }

        /// <summary>Kalan bekleme suresi (saniye).</summary>
        public int CooldownRemaining
        {
            get
            {
                int elapsed = Game.GameTime - _lastCallTime;
                int remaining = (_cfg.BackupCooldown - elapsed) / 1000;
                return remaining > 0 ? remaining : 0;
            }
        }

        public void Call(Func<Ped, bool> adoptCallback)
        {
            if (_stage != 0)
            {
                Utils.Notify("Takviye zaten yolda.");
                return;
            }

            if (CooldownRemaining > 0)
            {
                Utils.Notify("Takviye hazir degil (" + CooldownRemaining + " sn).");
                return;
            }

            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player)) return;

            try
            {
                // Oyuncunun arkasindan, yol uzerinde bir noktadan gelsinler.
                Vector3 origin = Utils.OffsetOf(player, new Vector3(0f, -_cfg.BackupSpawnDistance, 0f));
                Vector3 spot = Utils.StreetNode(origin);

                _vehicle = GuardFactory.CreateVehicle(_cfg, _cfg.BackupVehicleModels, spot,
                    Utils.HeadingTowards(spot, player.Position));

                if (!Utils.Valid(_vehicle))
                {
                    Utils.Notify("Takviye araci spawn edilemedi.");
                    return;
                }

                _driver = GuardFactory.CreateGuard(_cfg, spot, 0f);
                if (_driver == null)
                {
                    Utils.DeleteEntity(_vehicle);
                    _vehicle = null;
                    return;
                }

                N.SetIntoVehicle(_driver.Handle, _vehicle.Handle, -1);
                GuardFactory.ConfigureDriver(_driver, ConvoySpeedMode.Aggressive);

                _squad.Clear();
                int maxPassengers = N.GetMaxPassengers(_vehicle.Handle);
                int squadSize = Math.Min(_cfg.BackupSquadSize, maxPassengers);

                for (int seat = 0; seat < squadSize; seat++)
                {
                    Ped trooper = GuardFactory.CreateGuard(_cfg, spot, 0f);
                    if (trooper == null) break;

                    N.SetIntoVehicle(trooper.Handle, _vehicle.Handle, seat);
                    N.SetCombatAttribute(trooper.Handle, 3, false);
                    _squad.Add(trooper);
                }

                if (_cfg.EnableBlips)
                    _blip = Utils.AttachBlip(_vehicle, _cfg.ConvoyBlipSprite, _cfg.ConvoyBlipColor, "Takviye", 0.8f);

                // Oyuncuya dogru sur.
                N.TaskVehicleDriveToCoord(_driver.Handle, _vehicle.Handle, player.Position,
                    _cfg.SpeedAggressive, _cfg.DrivingStyleAggressive, 12f);

                _adoptCallback = adoptCallback;
                _stage = 1;
                _stageTime = Game.GameTime;
                _lastCallTime = Game.GameTime;
                _lastMovedTime = 0;

                Utils.Notify(string.Format("Takviye yolda: {0} birim.", _squad.Count));
                Logger.Info("Takviye cagrildi: " + _squad.Count + " birim.");
            }
            catch (Exception ex)
            {
                Logger.Error("BackupManager.Call hatasi", ex);
                Cleanup(true);
            }
        }

        public void Update()
        {
            if (_stage == 0) return;

            Ped player = Game.Player.Character;
            int now = Game.GameTime;

            if (!Utils.Valid(player)) return;

            // Arac imha edildiyse ekibi yerinde birak.
            if (!Utils.DrivableVehicle(_vehicle))
            {
                ReleaseSquadToPlayer();
                return;
            }

            // Zaman asimi (cagri anindan itibaren)
            if (now - _lastCallTime > 120000)
            {
                Utils.Notify("Takviye ulasamadi.");
                Cleanup(false);
                return;
            }

            float distance = _vehicle.Position.DistanceTo(player.Position);

            if (_stage == 1)
            {
                // Surucu oldu mu?
                if (!Utils.AlivePed(_driver))
                {
                    ReleaseSquadToPlayer();
                    return;
                }

                // Her 4 saniyede hedefi tazele (oyuncu hareket ediyor olabilir).
                if (now - _stageTime > 4000 && distance > 35f)
                {
                    N.TaskVehicleDriveToCoord(_driver.Handle, _vehicle.Handle, player.Position,
                        _cfg.SpeedAggressive, _cfg.DrivingStyleAggressive, 12f);
                    _stageTime = now - 3000;   // sik tetiklenmesin
                }

                CheckStuck(player, now, distance);

                if (distance < 30f)
                {
                    for (int i = 0; i < _squad.Count; i++)
                    {
                        Ped trooper = _squad[i];
                        if (!Utils.AlivePed(trooper)) continue;

                        N.SetCombatAttribute(trooper.Handle, 3, true);
                        N.TaskLeaveVehicle(trooper.Handle, _vehicle.Handle, 0);
                    }

                    _stage = 2;
                    _stageTime = now;
                    Utils.Notify("Takviye ulasti.");
                }
                return;
            }

            // _stage == 2: inis tamamlandi, ekibe kat.
            if (now - _stageTime > 3000)
                ReleaseSquadToPlayer();
        }

        /// <summary>Arac uzun sure yol alamiyorsa oyuncunun yakinina isinlayarak kurtarir.</summary>
        private void CheckStuck(Ped player, int now, float distanceToPlayer)
        {
            if (!Utils.DrivableVehicle(_vehicle)) return;

            Vector3 pos = _vehicle.Position;
            if (_lastMovedTime == 0)
            {
                _lastCheckPosition = pos;
                _lastMovedTime = now;
                return;
            }

            if (Utils.FlatDistance(pos, _lastCheckPosition) > 3f)
            {
                _lastCheckPosition = pos;
                _lastMovedTime = now;
                return;
            }

            bool weAreStopped = _vehicle.Speed < _cfg.StuckSpeedThreshold;

            if (weAreStopped && distanceToPlayer > 25f && now - _lastMovedTime > _cfg.StuckTimeThreshold)
            {
                Vector3 recover = Utils.StreetNode(Utils.OffsetOf(player, new Vector3(0f, -18f, 0f)));
                _vehicle.Position = recover;
                _vehicle.Heading = Utils.HeadingTowards(recover, player.Position);
                _vehicle.Velocity = Vector3.Zero;
                N.PlaceOnGroundProperly(_vehicle.Handle);

                _lastCheckPosition = _vehicle.Position;
                _lastMovedTime = now;
                _stageTime = now - 3000;
                Logger.Info("Takviye araci sikisti, oyuncunun yanina isinlandi.");
            }
        }

        private void ReleaseSquadToPlayer()
        {
            for (int i = 0; i < _squad.Count; i++)
            {
                Ped trooper = _squad[i];
                if (!Utils.AlivePed(trooper)) continue;

                N.SetCombatAttribute(trooper.Handle, 3, true);
                if (trooper.IsInVehicle() && Utils.Valid(_vehicle))
                    N.TaskLeaveVehicle(trooper.Handle, _vehicle.Handle, 0);

                bool adopted = _adoptCallback != null && _adoptCallback(trooper);
                if (!adopted)
                {
                    // Ekip doluysa serbest birak.
                    N.TaskWander(trooper.Handle);
                    Utils.Release(trooper);
                }
            }

            _squad.Clear();
            Cleanup(false);
            Utils.Notify("Takviye ekibi koruma kadrosuna katildi.");
        }

        private void Cleanup(bool deleteSquad)
        {
            Utils.RemoveBlip(_blip);
            _blip = null;

            if (deleteSquad)
            {
                for (int i = 0; i < _squad.Count; i++) Utils.DeleteEntity(_squad[i]);
                _squad.Clear();
            }

            // Surucu ve arac oyuna birakilir.
            if (Utils.AlivePed(_driver) && Utils.DrivableVehicle(_vehicle))
            {
                Ped player = Game.Player.Character;
                Vector3 away = Utils.Valid(player)
                    ? player.Position + new Vector3(250f, 250f, 0f)
                    : _vehicle.Position + new Vector3(250f, 250f, 0f);

                N.TaskVehicleDriveToCoord(_driver.Handle, _vehicle.Handle, away,
                    _cfg.SpeedNormal, _cfg.DrivingStyleNormal, 20f);
            }

            Utils.Release(_driver);
            Utils.Release(_vehicle);
            _driver = null;
            _vehicle = null;
            _stage = 0;
            _adoptCallback = null;
        }

        public void CleanupAll()
        {
            Utils.RemoveBlip(_blip);
            _blip = null;

            for (int i = 0; i < _squad.Count; i++) Utils.DeleteEntity(_squad[i]);
            _squad.Clear();

            Utils.DeleteEntity(_driver);
            Utils.DeleteEntity(_vehicle);
            _driver = null;
            _vehicle = null;
            _stage = 0;
            _adoptCallback = null;
        }
    }
}
