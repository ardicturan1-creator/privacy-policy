using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Hava savunma ve destek birimi.
    ///
    /// - Buzzard / Savage / Valkyrie / Annihilator vb. cagrilabilir
    /// - Pilot + nisanci ekibi dogru koltuklara oturur
    /// - Bos zamanda oyuncunun uzerinde daire cizer (mission 8)
    /// - Tehdit varsa hedefi engaje eder (mission 4 + nisancilara TASK_COMBAT_PED)
    /// - Cargobob ile extraction ve birlik indirme destegi
    /// </summary>
    internal sealed class AirSupport
    {
        // TASK_HELI_MISSION gorev kodlari
        private const int MissionAttack = 4;
        private const int MissionCircle = 8;
        private const int MissionLandNearPed = 20;

        private readonly Config _cfg;
        private readonly ThreatScanner _scanner;
        private readonly List<Ped> _gunners = new List<Ped>();

        private Vehicle _heli;
        private Ped _pilot;
        private Blip _blip;
        private int _lastTaskTime;
        private int _lastTargetHandle;
        private int _leaveTime;

        // --- Cargobob gorevleri ---
        private Vehicle _cargobob;
        private Ped _cargoPilot;
        private Blip _cargoBlip;
        private readonly List<Ped> _cargoTroops = new List<Ped>();
        private int _cargoStage;         // 0 yok, 1 yaklasiyor, 2 indi, 3 ayriliyor
        private int _cargoStageTime;
        private bool _cargoIsTroopDrop;
        private Func<Ped, bool> _adoptCallback;

        public AirSupport(Config cfg, ThreatScanner scanner)
        {
            _cfg = cfg;
            _scanner = scanner;
            Height = cfg.AirHeight;
            Radius = cfg.AirRadius;
            CurrentModel = cfg.DefaultAirVehicle;
            AutoEngage = cfg.AirAutoEngage;
            State = AirState.Idle;
        }

        // ------------------------------------------------------------------
        // Durum
        // ------------------------------------------------------------------
        public AirState State { get; private set; }
        public bool Active { get { return State != AirState.Idle && Utils.Valid(_heli); } }
        public float Height { get; private set; }
        public float Radius { get; private set; }
        public string CurrentModel { get; private set; }
        public bool AutoEngage { get; set; }
        public bool CargoBusy { get { return _cargoStage != 0; } }

        // ==================================================================
        // Cagirma / gonderme
        // ==================================================================
        public void Call(string modelName = null)
        {
            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player))
            {
                Utils.Notify("Hava destegi su anda cagrilamaz.");
                return;
            }

            if (!string.IsNullOrEmpty(modelName)) CurrentModel = modelName;

            // Onceki helikopteri gonder.
            if (Utils.Valid(_heli)) DismissInternal(true);

            try
            {
                // Oyuncunun arkasinda ve yukarisinda spawn.
                Vector3 spawn = Utils.OffsetOf(player, new Vector3(0f, -90f, Height + 25f));

                _heli = GuardFactory.CreateVehicle(_cfg, CurrentModel, spawn,
                    Utils.HeadingTowards(spawn, player.Position));

                if (!Utils.Valid(_heli))
                {
                    Utils.Notify("Helikopter spawn edilemedi: " + CurrentModel);
                    return;
                }

                N.SetHeliBladesFullSpeed(_heli.Handle);
                _heli.Velocity = _heli.ForwardVector * 20f;
                if (_cfg.AirInvincible) N.SetInvincible(_heli.Handle, true);

                // --- Pilot ---
                _pilot = GuardFactory.CreatePed(_cfg, _cfg.PilotModel, spawn, 0f);
                if (_pilot == null)
                {
                    Utils.DeleteEntity(_heli);
                    _heli = null;
                    Utils.Notify("Pilot spawn edilemedi.");
                    return;
                }

                N.SetIntoVehicle(_pilot.Handle, _heli.Handle, -1);
                N.SetCombatAttribute(_pilot.Handle, 1, true);    // arac kullanabilir
                N.SetCombatAttribute(_pilot.Handle, 3, false);   // helikopteri terk etmesin
                N.SetCombatAttribute(_pilot.Handle, 2, true);
                N.SetDriverAbility(_pilot.Handle, 1f);
                N.SetDriverAggressiveness(_pilot.Handle, 0.8f);

                // --- Nisancilar ---
                _gunners.Clear();
                int maxPassengers = N.GetMaxPassengers(_heli.Handle);
                int gunnerCount = Math.Min(_cfg.GunnerCount, maxPassengers);

                for (int seat = 0; seat < gunnerCount; seat++)
                {
                    Ped gunner = GuardFactory.CreateGuard(_cfg, spawn, 0f);
                    if (gunner == null) break;

                    N.SetIntoVehicle(gunner.Handle, _heli.Handle, seat);
                    N.SetCombatAttribute(gunner.Handle, 2, true);   // havadan ates
                    N.SetCombatAttribute(gunner.Handle, 3, false);  // atlamasin
                    N.SetCanBeKnockedOffVehicle(gunner.Handle, 1);
                    _gunners.Add(gunner);
                }

                if (_cfg.EnableBlips)
                {
                    _blip = Utils.AttachBlip(_heli, _cfg.AirBlipSprite, _cfg.AirBlipColor, "Hava Destegi", 0.9f, false);
                }

                State = AirState.Escorting;
                _lastTaskTime = 0;
                _lastTargetHandle = 0;

                Utils.Notify(string.Format("Hava destegi yolda: {0} ({1} nisanci).", CurrentModel, _gunners.Count));
                Logger.Info("Hava destegi cagrildi: " + CurrentModel);
            }
            catch (Exception ex)
            {
                Logger.Error("AirSupport.Call hatasi", ex);
                DismissInternal(true);
            }
        }

        public void Dismiss()
        {
            if (!Utils.Valid(_heli))
            {
                DismissInternal(true);
                Utils.Notify("Aktif hava destegi yok.");
                return;
            }

            // Ussune donsun: uzak bir noktaya ucur, sonra sil.
            Ped player = Game.Player.Character;
            if (Utils.AlivePed(_pilot) && Utils.Valid(player))
            {
                Vector3 away = player.Position + new Vector3(500f, 500f, 180f);
                N.TaskHeliMission(_pilot.Handle, _heli.Handle, 0, 0, away, MissionAttack, 60f, 30f, -1f, 200, 100);
            }

            Utils.RemoveBlip(_blip);
            _blip = null;
            State = AirState.Leaving;
            _leaveTime = Game.GameTime;
            Utils.Notify("Hava destegi ussune donuyor.");
        }

        public void Toggle()
        {
            if (Active) Dismiss();
            else Call();
        }

        /// <summary>Anlik ve sessiz temizlik.</summary>
        private void DismissInternal(bool deleteEntities)
        {
            Utils.RemoveBlip(_blip);
            _blip = null;

            for (int i = 0; i < _gunners.Count; i++)
            {
                if (deleteEntities) Utils.DeleteEntity(_gunners[i]);
                else Utils.Release(_gunners[i]);
            }
            _gunners.Clear();

            if (deleteEntities)
            {
                Utils.DeleteEntity(_pilot);
                Utils.DeleteEntity(_heli);
            }
            else
            {
                Utils.Release(_pilot);
                Utils.Release(_heli);
            }

            _pilot = null;
            _heli = null;
            State = AirState.Idle;
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        public void Update()
        {
            UpdateCargo();

            if (State == AirState.Idle) return;

            Ped player = Game.Player.Character;
            if (!Utils.Valid(player)) return;

            int now = Game.GameTime;

            // --- Ayrilma asamasi ---
            if (State == AirState.Leaving)
            {
                bool farEnough = !Utils.Valid(_heli) ||
                                 _heli.Position.DistanceTo(player.Position) > 400f;

                if (farEnough || now - _leaveTime > 25000)
                    DismissInternal(true);
                return;
            }

            // --- Helikopter imha edildi mi? ---
            if (!Utils.DrivableVehicle(_heli))
            {
                Utils.Notify("Hava destegi dusuruldu!");
                Logger.Warn("Hava destegi kaybedildi.");
                DismissInternal(false);
                return;
            }

            // --- Pilot oldu mu? Nisancilardan biri direksiyona gecsin. ---
            if (!Utils.AlivePed(_pilot))
            {
                if (!PromotePilot())
                {
                    Utils.Notify("Pilot oldu - helikopter kontrolden cikti!");
                    DismissInternal(false);
                    return;
                }
            }

            // --- Hedef secimi ---
            Ped target = null;
            if (AutoEngage && _scanner.HasThreats)
                target = _scanner.Nearest(_heli.Position);

            int targetHandle = Utils.AlivePed(target) ? target.Handle : 0;
            bool targetChanged = targetHandle != _lastTargetHandle;
            bool stale = now - _lastTaskTime > 5000;

            if (!targetChanged && !stale) return;

            if (targetHandle != 0)
            {
                // Saldiri gorevi
                N.TaskHeliMission(_pilot.Handle, _heli.Handle, 0, targetHandle, Vector3.Zero,
                    MissionAttack, _cfg.AirSpeed, 20f, -1f, (int)Height + 15, (int)Math.Max(15f, Height - 25f));

                for (int i = 0; i < _gunners.Count; i++)
                {
                    if (Utils.AlivePed(_gunners[i]))
                        N.TaskCombatPed(_gunners[i].Handle, targetHandle);
                }

                State = AirState.Engaging;
            }
            else
            {
                // Eskort: oyuncunun uzerinde daire ciz
                N.TaskHeliMission(_pilot.Handle, _heli.Handle, 0, player.Handle, Vector3.Zero,
                    MissionCircle, _cfg.AirSpeed, Radius, -1f, (int)Height, (int)Math.Max(15f, Height - 20f));

                State = AirState.Escorting;
            }

            _lastTargetHandle = targetHandle;
            _lastTaskTime = now;
        }

        private bool PromotePilot()
        {
            for (int i = 0; i < _gunners.Count; i++)
            {
                Ped gunner = _gunners[i];
                if (!Utils.AlivePed(gunner)) continue;

                N.SetIntoVehicle(gunner.Handle, _heli.Handle, -1);
                N.SetDriverAbility(gunner.Handle, 1f);
                _pilot = gunner;
                _gunners.RemoveAt(i);
                _lastTaskTime = 0;
                Logger.Info("Hava destegi: yardimci pilot devraldi.");
                return true;
            }
            return false;
        }

        // ------------------------------------------------------------------
        // Ayarlar
        // ------------------------------------------------------------------
        public void SetHeight(float height)
        {
            Height = Math.Max(15f, Math.Min(250f, height));
            _lastTaskTime = 0;
            Utils.Notify("Ucus yuksekligi: " + (int)Height + " m");
        }

        public void SetRadius(float radius)
        {
            Radius = Math.Max(20f, Math.Min(250f, radius));
            _lastTaskTime = 0;
            Utils.Notify("Devriye yaricapi: " + (int)Radius + " m");
        }

        public void SetModel(string modelName)
        {
            CurrentModel = modelName;
            if (Active)
            {
                Utils.Notify("Yeni helikopter cagriliyor: " + modelName);
                Call(modelName);
            }
            else
            {
                Utils.Notify("Hava araci secildi: " + modelName);
            }
        }

        // ==================================================================
        // Cargobob gorevleri
        // ==================================================================
        /// <summary>Cargobob ile tahliye: oyuncunun yanina iner ve bekler.</summary>
        public void RequestExtraction()
        {
            StartCargoMission(false, null);
        }

        /// <summary>Cargobob ile takviye birlik indirme.</summary>
        public void RequestTroopDrop(Func<Ped, bool> adoptCallback)
        {
            StartCargoMission(true, adoptCallback);
        }

        private void StartCargoMission(bool troopDrop, Func<Ped, bool> adoptCallback)
        {
            if (_cargoStage != 0)
            {
                Utils.Notify("Cargobob zaten gorevde.");
                return;
            }

            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player)) return;

            try
            {
                Vector3 spawn = Utils.OffsetOf(player, new Vector3(0f, -140f, 90f));
                _cargobob = GuardFactory.CreateVehicle(_cfg, _cfg.CargobobModel, spawn,
                    Utils.HeadingTowards(spawn, player.Position));

                if (!Utils.Valid(_cargobob))
                {
                    Utils.Notify("Cargobob spawn edilemedi: " + _cfg.CargobobModel);
                    return;
                }

                N.SetHeliBladesFullSpeed(_cargobob.Handle);
                _cargobob.Velocity = _cargobob.ForwardVector * 20f;

                _cargoPilot = GuardFactory.CreatePed(_cfg, _cfg.PilotModel, spawn, 0f);
                if (_cargoPilot == null)
                {
                    Utils.DeleteEntity(_cargobob);
                    _cargobob = null;
                    return;
                }

                N.SetIntoVehicle(_cargoPilot.Handle, _cargobob.Handle, -1);
                N.SetCombatAttribute(_cargoPilot.Handle, 3, false);
                N.SetDriverAbility(_cargoPilot.Handle, 1f);

                _cargoTroops.Clear();
                _cargoIsTroopDrop = troopDrop;
                _adoptCallback = adoptCallback;

                if (troopDrop)
                {
                    int maxPassengers = N.GetMaxPassengers(_cargobob.Handle);
                    int troopCount = Math.Min(_cfg.BackupSquadSize, maxPassengers);

                    for (int seat = 0; seat < troopCount; seat++)
                    {
                        Ped troop = GuardFactory.CreateGuard(_cfg, spawn, 0f);
                        if (troop == null) break;

                        N.SetIntoVehicle(troop.Handle, _cargobob.Handle, seat);
                        N.SetCombatAttribute(troop.Handle, 3, false);
                        _cargoTroops.Add(troop);
                    }
                }

                if (_cfg.EnableBlips)
                    _cargoBlip = Utils.AttachBlip(_cargobob, _cfg.AirBlipSprite, _cfg.AirBlipColor,
                        troopDrop ? "Takviye Ucusu" : "Tahliye Ucusu", 0.9f, false);

                // Oyuncunun yanina in.
                N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, player.Handle, Vector3.Zero,
                    MissionLandNearPed, 60f, 40f, -1f, 60, 20);

                _cargoStage = 1;
                _cargoStageTime = Game.GameTime;

                Utils.Notify(troopDrop ? "Takviye birligi hava yoluyla geliyor." : "Tahliye helikopteri yolda.");
            }
            catch (Exception ex)
            {
                Logger.Error("StartCargoMission hatasi", ex);
                CleanupCargo(true);
            }
        }

        private void UpdateCargo()
        {
            if (_cargoStage == 0) return;

            Ped player = Game.Player.Character;
            int now = Game.GameTime;

            if (!Utils.DrivableVehicle(_cargobob) || !Utils.AlivePed(_cargoPilot))
            {
                Utils.Notify("Cargobob gorevi basarisiz oldu.");
                CleanupCargo(false);
                return;
            }

            // Zaman asimi guvenligi
            if (now - _cargoStageTime > 120000)
            {
                CleanupCargo(false);
                return;
            }

            switch (_cargoStage)
            {
                case 1:
                    {
                        bool nearGround = _cargobob.HeightAboveGround < 6f;
                        bool closeToPlayer = Utils.Valid(player) &&
                                             Utils.FlatDistance(_cargobob.Position, player.Position) < 45f;

                        if (nearGround && closeToPlayer)
                        {
                            _cargoStage = 2;
                            _cargoStageTime = now;

                            if (_cargoIsTroopDrop)
                            {
                                for (int i = 0; i < _cargoTroops.Count; i++)
                                {
                                    Ped troop = _cargoTroops[i];
                                    if (!Utils.AlivePed(troop)) continue;

                                    N.SetCombatAttribute(troop.Handle, 3, true);
                                    N.TaskLeaveVehicle(troop.Handle, _cargobob.Handle, 0);
                                }
                                Utils.Notify("Takviye birligi indi.");
                            }
                            else
                            {
                                Utils.Notify("Tahliye helikopteri indi - binebilirsiniz.");
                            }
                        }
                        break;
                    }

                case 2:
                    {
                        if (_cargoIsTroopDrop)
                        {
                            // Askerler indikten sonra yakin koruma ekibine katilsin.
                            if (now - _cargoStageTime > 3500)
                            {
                                for (int i = 0; i < _cargoTroops.Count; i++)
                                {
                                    Ped troop = _cargoTroops[i];
                                    if (!Utils.AlivePed(troop)) continue;

                                    bool adopted = _adoptCallback != null && _adoptCallback(troop);
                                    if (!adopted) Utils.Release(troop);
                                }
                                _cargoTroops.Clear();
                                DepartCargo(player);
                            }
                        }
                        else
                        {
                            // Oyuncu bindiyse havalan.
                            if (Utils.Valid(player) && player.IsInVehicle(_cargobob))
                            {
                                Utils.Notify("Tahliye basladi.");
                                if (Utils.AlivePed(_cargoPilot))
                                {
                                    N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, 0,
                                        player.Position + new Vector3(400f, 400f, 150f),
                                        MissionAttack, 50f, 30f, -1f, 150, 60);
                                }
                                Utils.RemoveBlip(_cargoBlip);
                                _cargoBlip = null;
                                _cargoStage = 0;
                                _cargoTroops.Clear();

                                // Oyuncu iceride oldugu icin araci silmiyoruz, sadece birakiyoruz.
                                Utils.Release(_cargobob);
                                Utils.Release(_cargoPilot);
                                _cargobob = null;
                                _cargoPilot = null;
                            }
                            else if (now - _cargoStageTime > 60000)
                            {
                                Utils.Notify("Tahliye helikopteri bekleyemedi, ayriliyor.");
                                DepartCargo(player);
                            }
                        }
                        break;
                    }

                case 3:
                    {
                        bool farEnough = !Utils.Valid(player) ||
                                         _cargobob.Position.DistanceTo(player.Position) > 350f;

                        if (farEnough || now - _cargoStageTime > 40000)
                            CleanupCargo(true);
                        break;
                    }
            }
        }

        private void DepartCargo(Ped player)
        {
            if (Utils.AlivePed(_cargoPilot) && Utils.Valid(_cargobob))
            {
                Vector3 away = (Utils.Valid(player) ? player.Position : _cargobob.Position)
                               + new Vector3(400f, 400f, 160f);
                N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, 0, away,
                    MissionAttack, 55f, 30f, -1f, 180, 80);
            }

            Utils.RemoveBlip(_cargoBlip);
            _cargoBlip = null;
            _cargoStage = 3;
            _cargoStageTime = Game.GameTime;
        }

        private void CleanupCargo(bool deleteEntities)
        {
            Utils.RemoveBlip(_cargoBlip);
            _cargoBlip = null;

            for (int i = 0; i < _cargoTroops.Count; i++)
            {
                if (deleteEntities) Utils.DeleteEntity(_cargoTroops[i]);
                else Utils.Release(_cargoTroops[i]);
            }
            _cargoTroops.Clear();

            if (deleteEntities)
            {
                Utils.DeleteEntity(_cargoPilot);
                Utils.DeleteEntity(_cargobob);
            }
            else
            {
                Utils.Release(_cargoPilot);
                Utils.Release(_cargobob);
            }

            _cargoPilot = null;
            _cargobob = null;
            _cargoStage = 0;
            _adoptCallback = null;
        }

        // ------------------------------------------------------------------
        // Temizlik
        // ------------------------------------------------------------------
        public void CleanupAll()
        {
            DismissInternal(true);
            CleanupCargo(true);
        }
    }
}
