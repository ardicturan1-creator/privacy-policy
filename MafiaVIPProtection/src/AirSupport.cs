using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Hava savunma ve destek birimi — COKLU birim destekli.
    ///
    /// Onemli gercekler (resmi TASK_HELI_MISSION / SET_PED_COMBAT_ATTRIBUTES
    /// dokumantasyonuyla dogrulandi):
    ///
    /// 1) Silahli helikopterlerde (Buzzard, Savage, Annihilator, Hunter...) mermiyi
    ///    SADECE PILOT atesler; yolcu koltugundaki bir ped monteli silahi kontrol
    ///    edemez. Bu yuzden "nisanci" kavramimiz gercekci: mürettebat, helikopter
    ///    dusurulurse yerde savasan ek destek olarak vardir; asil ates gucu pilotun
    ///    Attack (6) gorevine otomatik olarak baglidir.
    ///
    /// 2) TASK_HELI_MISSION'in gorev tipi (missionType) TASK_VEHICLE_MISSION ile
    ///    AYNI numaralandirmayi paylasir: GoTo=4, Attack=6, Circle=9, Land=19,
    ///    LandAndWait=20. Internette Circle=8 / Attack=4 diye yanlis kopyalanir;
    ///    8 aslinda Flee'dir — yanlis kullanilirsa helikopter oyuncudan KACAR.
    ///
    /// 3) Inis gorevlerinde missionFlags parametresine LandOnArrival(32) |
    ///    DontDoAvoidance(64) verilmezse pilot AI'i hedefe yaklasir ama hicbir
    ///    zaman yere degmez, sonsuza kadar havada asili kalir (bu, "tahliyeye
    ///    binilmiyor" sikayetinin kok nedenidir). Ayrica AI inis gorevi bazen
    ///    engebeli/dar alanlarda basarisiz olabilecegi icin bir zaman asimindan
    ///    sonra manuel "yere kilitleme" (freeze) yedek mekanizmasi da var.
    /// </summary>
    internal sealed class AirSupport
    {
        private sealed class AirUnit
        {
            public Vehicle Heli;
            public Ped Pilot;
            public readonly List<Ped> Crew = new List<Ped>();
            public Blip Blip;
            public AirState State = AirState.Escorting;
            public string Model;
            public int Index;
            public int LastTaskTime;
            public int LastTargetHandle;
            public int LeaveStartTime;
        }

        private readonly Config _cfg;
        private readonly ThreatScanner _scanner;
        private readonly List<AirUnit> _units = new List<AirUnit>();
        private readonly List<Ped> _pilotView = new List<Ped>();
        private int _unitCounter;

        // --- Cargobob gorevleri (tek seferlik, tahliye/birlik indirme) ---
        private Vehicle _cargobob;
        private Ped _cargoPilot;
        private Blip _cargoBlip;
        private readonly List<Ped> _cargoTroops = new List<Ped>();
        private int _cargoStage;         // 0 yok, 1 yaklasiyor, 2 indi, 3 ayriliyor
        private int _cargoStageTime;
        private int _cargoApproachStart;
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
        }

        // ------------------------------------------------------------------
        // Durum
        // ------------------------------------------------------------------
        public bool Active { get { return _units.Count > 0; } }
        public int UnitCount { get { return _units.Count; } }
        public int MaxUnits { get { return _cfg.MaxAirUnits; } }
        public float Height { get; private set; }
        public float Radius { get; private set; }
        public string CurrentModel { get; private set; }
        public bool AutoEngage { get; set; }
        public bool CargoBusy { get { return _cargoStage != 0; } }

        // ==================================================================
        // Cagirma / gonderme
        // ==================================================================
        /// <summary>Yeni bir hava birimi ekler (MaxAirUnits'e kadar). Mevcutlari degistirmez.</summary>
        public void CallUnit(string modelName = null)
        {
            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player))
            {
                Utils.Notify("Hava destegi su anda cagrilamaz.");
                return;
            }

            if (_units.Count >= _cfg.MaxAirUnits)
            {
                Utils.Notify(string.Format("Hava destegi limiti doldu ({0}/{0}). Once bir birimi geri gonderin.",
                    _cfg.MaxAirUnits));
                return;
            }

            if (!string.IsNullOrEmpty(modelName)) CurrentModel = modelName;
            string model = CurrentModel;

            try
            {
                int index = _units.Count;
                // Her birim farkli bir noktadan gelsin ki cakismasinlar.
                float angle = index * 47f;
                Vector3 spawn = Utils.OffsetOf(player, new Vector3(
                    (float)Math.Sin(angle * Math.PI / 180.0) * 90f,
                    -70f - index * 25f,
                    Height + 25f + index * 10f));

                Vehicle heli = GuardFactory.CreateVehicle(_cfg, model, spawn, Utils.HeadingTowards(spawn, player.Position));
                if (!Utils.Valid(heli))
                {
                    Utils.Notify("Helikopter spawn edilemedi: " + model);
                    return;
                }

                N.SetHeliBladesFullSpeed(heli.Handle);
                heli.Velocity = heli.ForwardVector * 20f;
                if (_cfg.AirInvincible) N.SetInvincible(heli.Handle, true);

                Ped pilot = GuardFactory.CreatePed(_cfg, _cfg.PilotModel, spawn, 0f);
                if (pilot == null)
                {
                    Utils.DeleteEntity(heli);
                    Utils.Notify("Pilot spawn edilemedi.");
                    return;
                }

                N.SetIntoVehicle(pilot.Handle, heli.Handle, -1);
                N.SetCombatAttribute(pilot.Handle, 1, true);
                N.SetCombatAttribute(pilot.Handle, 2, true);
                N.SetCombatAttribute(pilot.Handle, 3, false);
                N.SetDriverAbility(pilot.Handle, 1f);
                N.SetDriverAggressiveness(pilot.Handle, 0.85f);

                AirUnit unit = new AirUnit
                {
                    Heli = heli,
                    Pilot = pilot,
                    Model = model,
                    Index = index,
                    State = AirState.Escorting
                };

                int maxPassengers = N.GetMaxPassengers(heli.Handle);
                int crewCount = Math.Min(_cfg.GunnerCount, maxPassengers);

                for (int seat = 0; seat < crewCount; seat++)
                {
                    Ped crew = GuardFactory.CreateGuard(_cfg, spawn, 0f);
                    if (crew == null) break;

                    N.SetIntoVehicle(crew.Handle, heli.Handle, seat);
                    N.SetCombatAttribute(crew.Handle, 2, true);
                    N.SetCombatAttribute(crew.Handle, 3, false);
                    N.SetCanBeKnockedOffVehicle(crew.Handle, 1);
                    unit.Crew.Add(crew);
                }

                if (_cfg.EnableBlips)
                {
                    _unitCounter++;
                    unit.Blip = Utils.AttachBlip(heli, _cfg.AirBlipSprite, _cfg.AirBlipColor,
                        "Hava Destegi #" + _unitCounter, 0.9f, false);
                }

                _units.Add(unit);
                Utils.Notify(string.Format("Hava destegi yolda ({0}/{1}): {2}.",
                    _units.Count, _cfg.MaxAirUnits, model));
                Logger.Info("Hava destegi birimi cagrildi: " + model);
            }
            catch (Exception ex)
            {
                Logger.Error("AirSupport.CallUnit hatasi", ex);
            }
        }

        /// <summary>En son cagrilan birimi ussune gonderir.</summary>
        public void DismissLast()
        {
            if (_units.Count == 0)
            {
                Utils.Notify("Aktif hava destegi yok.");
                return;
            }
            DismissUnit(_units[_units.Count - 1]);
        }

        public void DismissAll()
        {
            if (_units.Count == 0)
            {
                Utils.Notify("Aktif hava destegi yok.");
                return;
            }

            for (int i = 0; i < _units.Count; i++) DismissUnit(_units[i]);
            Utils.Notify("Tum hava destegi ussune donuyor.");
        }

        private void DismissUnit(AirUnit unit)
        {
            if (unit.State == AirState.Leaving) return;

            Ped player = Game.Player.Character;
            if (Utils.AlivePed(unit.Pilot) && Utils.DrivableVehicle(unit.Heli) && Utils.Valid(player))
            {
                Vector3 away = player.Position + new Vector3(500f, 500f, 180f + unit.Index * 20f);
                N.TaskHeliMission(unit.Pilot.Handle, unit.Heli.Handle, 0, 0, away,
                    HeliMission.GoTo, 60f, 30f, -1f, 220, 120);
            }

            Utils.RemoveBlip(unit.Blip);
            unit.Blip = null;
            unit.State = AirState.Leaving;
            unit.LeaveStartTime = Game.GameTime;
        }

        /// <summary>Tum birimleri aninda ve sessizce siler (script kapanisi vb.).</summary>
        private void PurgeUnit(AirUnit unit, bool deleteEntities)
        {
            Utils.RemoveBlip(unit.Blip);
            unit.Blip = null;

            for (int i = 0; i < unit.Crew.Count; i++)
            {
                if (deleteEntities) Utils.DeleteEntity(unit.Crew[i]);
                else Utils.Release(unit.Crew[i]);
            }
            unit.Crew.Clear();

            if (deleteEntities)
            {
                Utils.DeleteEntity(unit.Pilot);
                Utils.DeleteEntity(unit.Heli);
            }
            else
            {
                Utils.Release(unit.Pilot);
                Utils.Release(unit.Heli);
            }
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        public void Update()
        {
            UpdateCargo();

            if (_units.Count == 0) return;

            Ped player = Game.Player.Character;
            if (!Utils.Valid(player)) return;

            int now = Game.GameTime;

            // --- Ayrilanlari temizle ---
            for (int i = _units.Count - 1; i >= 0; i--)
            {
                AirUnit unit = _units[i];

                if (unit.State == AirState.Leaving)
                {
                    bool farEnough = !Utils.Valid(unit.Heli) ||
                                     unit.Heli.Position.DistanceTo(player.Position) > 400f;

                    if (farEnough || now - unit.LeaveStartTime > 25000)
                    {
                        PurgeUnit(unit, true);
                        _units.RemoveAt(i);
                    }
                    continue;
                }

                if (!Utils.DrivableVehicle(unit.Heli))
                {
                    Utils.Notify("Bir hava destegi birimi dusuruldu!");
                    Logger.Warn("Hava destegi birimi kaybedildi.");
                    PurgeUnit(unit, false);
                    _units.RemoveAt(i);
                    continue;
                }

                if (!Utils.AlivePed(unit.Pilot) && !PromotePilot(unit))
                {
                    Utils.Notify("Bir hava destegi biriminin pilotu oldu, kontrolden cikti.");
                    PurgeUnit(unit, false);
                    _units.RemoveAt(i);
                }
            }

            if (_units.Count == 0) return;

            // --- Hedef dagitimi: birden fazla birim ayni tehdide uşuşmesin ---
            _pilotView.Clear();
            for (int i = 0; i < _units.Count; i++)
            {
                if (_units[i].State == AirState.Leaving) continue;
                if (Utils.AlivePed(_units[i].Pilot)) _pilotView.Add(_units[i].Pilot);
            }

            Dictionary<int, Ped> assignment = (AutoEngage && _scanner.HasThreats)
                ? _scanner.AssignTargets(_pilotView)
                : null;

            for (int i = 0; i < _units.Count; i++)
            {
                AirUnit unit = _units[i];
                if (unit.State == AirState.Leaving) continue;

                try { UpdateUnit(unit, player, assignment, now); }
                catch (Exception ex) { Logger.Error("Hava birimi guncelleme hatasi", ex); }
            }
        }

        private void UpdateUnit(AirUnit unit, Ped player, Dictionary<int, Ped> assignment, int now)
        {
            Ped target = null;
            if (assignment != null && Utils.AlivePed(unit.Pilot))
                assignment.TryGetValue(unit.Pilot.Handle, out target);

            if (!Utils.AlivePed(target)) target = null;

            int targetHandle = target != null ? target.Handle : 0;
            bool targetChanged = targetHandle != unit.LastTargetHandle;
            bool stale = now - unit.LastTaskTime > 5000;

            if (!targetChanged && !stale) return;

            if (targetHandle != 0)
            {
                // Saldiri: pilot AI'i monteli silahi otomatik kullanir.
                N.TaskHeliMission(unit.Pilot.Handle, unit.Heli.Handle, 0, targetHandle, Vector3.Zero,
                    HeliMission.Attack, _cfg.AirSpeed, 20f, -1f,
                    (int)Height + 15, (int)Math.Max(15f, Height - 25f));

                for (int i = 0; i < unit.Crew.Count; i++)
                {
                    if (Utils.AlivePed(unit.Crew[i]))
                        N.TaskCombatPed(unit.Crew[i].Handle, targetHandle);
                }

                unit.State = AirState.Engaging;
            }
            else
            {
                // Bos zamanda oyuncunun uzerinde daire ciz. Birden fazla birim
                // ayni cemberde cakismasin diye yaricap/yukseklik kademelendirilir
                // ve yon sirayla tersine cevrilir.
                float radius = Radius + unit.Index * 18f;
                int height = (int)Height + unit.Index * 12;
                int flags = (unit.Index % 2 == 1) ? HeliMissionFlags.None | 2048 /* CircleOppositeDirection */ : HeliMissionFlags.None;

                N.TaskHeliMission(unit.Pilot.Handle, unit.Heli.Handle, 0, player.Handle, Vector3.Zero,
                    HeliMission.Circle, _cfg.AirSpeed, radius, -1f,
                    height, Math.Max(15, height - 20), flags);

                unit.State = AirState.Escorting;
            }

            unit.LastTargetHandle = targetHandle;
            unit.LastTaskTime = now;
        }

        private bool PromotePilot(AirUnit unit)
        {
            for (int i = 0; i < unit.Crew.Count; i++)
            {
                Ped candidate = unit.Crew[i];
                if (!Utils.AlivePed(candidate)) continue;

                N.SetIntoVehicle(candidate.Handle, unit.Heli.Handle, -1);
                N.SetDriverAbility(candidate.Handle, 1f);
                N.SetCombatAttribute(candidate.Handle, 1, true);
                N.SetCombatAttribute(candidate.Handle, 3, false);
                unit.Pilot = candidate;
                unit.Crew.RemoveAt(i);
                unit.LastTaskTime = 0;
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
            for (int i = 0; i < _units.Count; i++) _units[i].LastTaskTime = 0;
            Utils.Notify("Ucus yuksekligi: " + (int)Height + " m");
        }

        public void SetRadius(float radius)
        {
            Radius = Math.Max(20f, Math.Min(250f, radius));
            for (int i = 0; i < _units.Count; i++) _units[i].LastTaskTime = 0;
            Utils.Notify("Devriye yaricapi: " + (int)Radius + " m");
        }

        public void SetModel(string modelName)
        {
            CurrentModel = modelName;
            Utils.Notify("Bir sonraki cagrida kullanilacak arac: " + modelName);
        }

        // ==================================================================
        // Cargobob gorevleri
        // ==================================================================
        public void RequestExtraction()
        {
            StartCargoMission(false, null);
        }

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

                // Oyuncunun yanina in: LandAndWait + LandOnArrival|DontDoAvoidance
                // olmadan pilot hedefe yaklasir ama asla yere degmez.
                N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, player.Handle, Vector3.Zero,
                    HeliMission.LandAndWait, 60f, 40f, -1f, 60, 20, HeliMissionFlags.LandingCombo);

                _cargoStage = 1;
                _cargoStageTime = Game.GameTime;
                _cargoApproachStart = Game.GameTime;

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

            if (now - _cargoStageTime > 150000)
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

                        // AI inisi basarisizsa (dar alan, engebeli arazi vb.) belirli bir
                        // sureden sonra manuel olarak zemine kilitle — boylece oyuncu
                        // helikopterin sonsuza dek havada asili kalmasi yuzunden asla
                        // binemedigi durumla karsilasmaz.
                        bool forceLanded = false;
                        if (!nearGround && closeToPlayer && now - _cargoApproachStart > _cfg.LandingTimeout)
                        {
                            Vector3 ground = Utils.SafeGround(_cargobob.Position);
                            _cargobob.Position = new Vector3(_cargobob.Position.X, _cargobob.Position.Y, ground.Z + 0.6f);
                            _cargobob.Velocity = Vector3.Zero;
                            _cargobob.IsPositionFrozen = true;
                            forceLanded = true;
                            Logger.Info("Cargobob manuel inis ile zemine kilitlendi (AI inis zaman asimi).");
                        }

                        if ((nearGround && closeToPlayer) || forceLanded)
                        {
                            _cargobob.IsPositionFrozen = true;
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
                            if (Utils.Valid(player) && player.IsInVehicle(_cargobob))
                            {
                                Utils.Notify("Tahliye basladi.");
                                _cargobob.IsPositionFrozen = false;

                                if (Utils.AlivePed(_cargoPilot))
                                {
                                    N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, 0,
                                        player.Position + new Vector3(400f, 400f, 150f),
                                        HeliMission.GoTo, 50f, 30f, -1f, 150, 60);
                                }
                                Utils.RemoveBlip(_cargoBlip);
                                _cargoBlip = null;
                                _cargoStage = 0;
                                _cargoTroops.Clear();

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
            _cargobob.IsPositionFrozen = false;

            if (Utils.AlivePed(_cargoPilot) && Utils.Valid(_cargobob))
            {
                Vector3 away = (Utils.Valid(player) ? player.Position : _cargobob.Position)
                               + new Vector3(400f, 400f, 160f);
                N.TaskHeliMission(_cargoPilot.Handle, _cargobob.Handle, 0, 0, away,
                    HeliMission.GoTo, 55f, 30f, -1f, 180, 80);
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

            if (Utils.Valid(_cargobob)) _cargobob.IsPositionFrozen = false;

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
            for (int i = 0; i < _units.Count; i++) PurgeUnit(_units[i], true);
            _units.Clear();
            CleanupCargo(true);
        }
    }
}
