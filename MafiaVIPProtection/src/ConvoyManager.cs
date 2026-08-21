using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// Konvoy / motorcade sistemi.
    ///
    /// - VIP araci (spawn edilen zirhli arac veya oyuncunun mevcut araci)
    /// - Onde lead araclar, arkada / yanlarda destek araclari
    /// - TASK_VEHICLE_ESCORT ile gercekci dizilim (virajda dagilmaz)
    /// - Konvoy durunca ekip inip 360 derece savunma alir
    /// - Oyuncu baska araca gecerse konvoy yeni araci korumaya devam eder
    /// </summary>
    internal sealed class ConvoyManager
    {
        private sealed class ConvoyUnit
        {
            public Vehicle Vehicle;
            public Ped Driver;
            public readonly List<Ped> Crew = new List<Ped>();
            public Blip Blip;
            public int Role = EscortMode.Rear;
            public float DistanceOffset;
            public int LastTaskTime;
            public int LastCombatTime;
            public int LastTargetHandle;
            public int LastMode = -999;

            // Sikisma tespiti icin
            public Vector3 LastCheckPosition;
            public int LastMovedTime;
        }

        private readonly Config _cfg;
        private readonly ThreatScanner _scanner;
        private readonly List<ConvoyUnit> _units = new List<ConvoyUnit>();

        private Vehicle _vipVehicle;
        private Blip _vipBlip;
        private bool _ownsVipVehicle;

        private int _deployStage;        // 0 = araclarda, 1 = iniyor, 2 = mevzide
        private int _deployStageTime;
        private int _stoppedSince;
        private int _movingSince;

        public ConvoyManager(Config cfg, ThreatScanner scanner)
        {
            _cfg = cfg;
            _scanner = scanner;
            SpeedMode = ConvoySpeedMode.Normal;
            ConvoyFormation = Formation.Diamond;
            Behavior = cfg.DefaultBehavior;
        }

        // ------------------------------------------------------------------
        // Durum
        // ------------------------------------------------------------------
        public bool Active { get; private set; }
        public ConvoySpeedMode SpeedMode { get; private set; }
        public Formation ConvoyFormation { get; private set; }
        public GuardBehavior Behavior { get; set; }
        public int UnitCount { get { return _units.Count; } }
        public Vehicle VipVehicle { get { return _vipVehicle; } }
        public bool Deployed { get { return _deployStage > 0; } }

        private float CurrentSpeed
        {
            get
            {
                switch (SpeedMode)
                {
                    case ConvoySpeedMode.Aggressive: return _cfg.SpeedAggressive;
                    case ConvoySpeedMode.Stealth: return _cfg.SpeedStealth;
                    default: return _cfg.SpeedNormal;
                }
            }
        }

        private int CurrentDrivingStyle
        {
            get
            {
                switch (SpeedMode)
                {
                    case ConvoySpeedMode.Aggressive: return _cfg.DrivingStyleAggressive;
                    case ConvoySpeedMode.Stealth: return _cfg.DrivingStyleStealth;
                    default: return _cfg.DrivingStyleNormal;
                }
            }
        }

        // ==================================================================
        // Kurulum
        // ==================================================================
        public void Create()
        {
            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player))
            {
                Utils.Notify("Konvoy su anda olusturulamaz.");
                return;
            }

            Dismiss(true);

            try
            {
                PrepareVipVehicle(player);

                Entity anchor = Utils.Valid(_vipVehicle) ? (Entity)_vipVehicle : player;
                float heading = anchor.Heading;

                // --- Lead araclar (onde) ---
                for (int i = 0; i < _cfg.LeadVehicleCount; i++)
                {
                    Vector3 spot = Utils.StreetNode(Utils.OffsetOf(anchor, new Vector3(0f, 14f + i * 9f, 0f)));
                    ConvoyUnit unit = SpawnUnit(_cfg.LeadVehicleModels, spot, heading, EscortMode.Front, 8f + i * 6f);
                    if (unit != null) _units.Add(unit);
                }

                // --- Destek araclar (arkada / yanlarda) ---
                for (int i = 0; i < _cfg.SupportVehicleCount; i++)
                {
                    Vector3 spot = Utils.StreetNode(Utils.OffsetOf(anchor, new Vector3(0f, -14f - i * 9f, 0f)));
                    ConvoyUnit unit = SpawnUnit(_cfg.SupportVehicleModels, spot, heading, EscortMode.Rear, 10f + i * 6f);
                    if (unit != null) _units.Add(unit);
                }

                if (_units.Count == 0)
                {
                    Utils.Notify("Konvoy araci spawn edilemedi. Model isimlerini kontrol edin.");
                    Dismiss(true);
                    return;
                }

                AssignRoles();
                Active = true;
                _deployStage = 0;
                _stoppedSince = 0;
                _movingSince = Game.GameTime;

                Utils.Notify(string.Format("Konvoy hazir: {0} eskort araci, mod {1}.",
                    _units.Count, SpeedModeName(SpeedMode)));
                Logger.Info("Konvoy olusturuldu: " + _units.Count + " arac.");
            }
            catch (Exception ex)
            {
                Logger.Error("ConvoyManager.Create hatasi", ex);
                Dismiss(true);
            }
        }

        private void PrepareVipVehicle(Ped player)
        {
            // Oyuncu zaten bir aractaysa ve ayar kapaliysa onu VIP araci say.
            if (!_cfg.SpawnVipVehicle && player.IsInVehicle())
            {
                _vipVehicle = player.CurrentVehicle;
                _ownsVipVehicle = false;
            }
            else
            {
                Vector3 spot = Utils.StreetNode(Utils.OffsetOf(player, new Vector3(0f, 6f, 0f)));
                _vipVehicle = GuardFactory.CreateVehicle(_cfg, _cfg.VipVehicleModel, spot, player.Heading);
                _ownsVipVehicle = Utils.Valid(_vipVehicle);

                if (Utils.Valid(_vipVehicle))
                {
                    if (_cfg.WarpPlayerIntoVip)
                        N.SetIntoVehicle(player.Handle, _vipVehicle.Handle, -1);
                    else
                        Utils.Notify("VIP araci hazir.");
                }
                else if (player.IsInVehicle())
                {
                    // Yedek plan: mevcut araci VIP kabul et.
                    _vipVehicle = player.CurrentVehicle;
                    _ownsVipVehicle = false;
                }
            }

            if (_cfg.EnableBlips && Utils.Valid(_vipVehicle))
            {
                Utils.RemoveBlip(_vipBlip);
                _vipBlip = Utils.AttachBlip(_vipVehicle, _cfg.ConvoyBlipSprite, _cfg.ConvoyBlipColor, "VIP Araci", 0.9f, false);
            }
        }

        private ConvoyUnit SpawnUnit(string[] models, Vector3 position, float heading, int role, float distanceOffset)
        {
            Vehicle vehicle = GuardFactory.CreateVehicle(_cfg, models, position, heading);
            if (!Utils.Valid(vehicle)) return null;

            ConvoyUnit unit = new ConvoyUnit
            {
                Vehicle = vehicle,
                Role = role,
                DistanceOffset = distanceOffset
            };

            // Surucu
            Ped driver = GuardFactory.CreateGuard(_cfg, position, heading);
            if (driver == null)
            {
                Utils.DeleteEntity(vehicle);
                return null;
            }

            N.SetIntoVehicle(driver.Handle, vehicle.Handle, -1);
            GuardFactory.ConfigureDriver(driver, SpeedMode);
            unit.Driver = driver;

            // Ekip
            int maxPassengers = N.GetMaxPassengers(vehicle.Handle);
            int crewSize = Math.Min(_cfg.GuardsPerVehicle, maxPassengers);

            for (int seat = 0; seat < crewSize; seat++)
            {
                Ped crew = GuardFactory.CreateGuard(_cfg, position, heading);
                if (crew == null) break;

                N.SetIntoVehicle(crew.Handle, vehicle.Handle, seat);
                N.SetCombatAttribute(crew.Handle, 2, true);   // drive-by
                N.SetCombatAttribute(crew.Handle, 3, false);  // kendiliginden inmesin
                unit.Crew.Add(crew);
            }

            if (_cfg.EnableBlips)
            {
                unit.Blip = Utils.AttachBlip(vehicle, _cfg.ConvoyBlipSprite, _cfg.ConvoyBlipColor,
                    role == EscortMode.Front ? "Konvoy (Onde)" : "Konvoy (Destek)", 0.75f);
            }

            return unit;
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        public void Update()
        {
            if (!Active) return;

            Ped player = Game.Player.Character;
            if (!Utils.Valid(player)) return;

            PruneDeadUnits();

            if (_units.Count == 0)
            {
                Utils.Notify("Konvoy imha edildi.");
                Dismiss(true);
                return;
            }

            Entity target = ResolveTarget(player);
            if (!Utils.Valid(target)) return;

            UpdateHaltState(target);

            if (_deployStage > 0)
            {
                UpdateDeployedCrew(target);
                return;
            }

            int now = Game.GameTime;

            // Her aracin surucusunu "temsilci" olarak kullanip hedefleri dagit;
            // boylece tum konvoy tek bir dusmana yigilmaz.
            List<Ped> representatives = new List<Ped>();
            for (int i = 0; i < _units.Count; i++)
                if (Utils.AlivePed(_units[i].Driver)) representatives.Add(_units[i].Driver);

            Dictionary<int, Ped> assignment = (Behavior != GuardBehavior.Passive && _scanner.HasThreats)
                ? _scanner.AssignTargets(representatives)
                : null;

            for (int i = 0; i < _units.Count; i++)
            {
                ConvoyUnit unit = _units[i];
                try
                {
                    UpdateUnit(unit, target, assignment, now);
                    CheckStuck(unit, target, now);
                }
                catch (Exception ex) { Logger.Error("Konvoy birimi guncelleme hatasi", ex); }
            }
        }

        /// <summary>
        /// Hedef hareket ederken arac uzun sure neredeyse hic ilerlemiyorsa
        /// (bir binaya/geometriye takilma, kotu pathing vb.) araci hedefin
        /// yakinina isinlayarak kurtarir. Konvoyun "sikisip kalmasi" sikayetinin
        /// dogrudan cozumu.
        /// </summary>
        private void CheckStuck(ConvoyUnit unit, Entity target, int now)
        {
            if (!Utils.DrivableVehicle(unit.Vehicle) || !Utils.AlivePed(unit.Driver)) return;
            if (_deployStage > 0) return;   // mevzideyken sikisma kontrolu yapma

            Vector3 pos = unit.Vehicle.Position;

            if (unit.LastMovedTime == 0)
            {
                unit.LastCheckPosition = pos;
                unit.LastMovedTime = now;
                return;
            }

            if (Utils.FlatDistance(pos, unit.LastCheckPosition) > 3f)
            {
                unit.LastCheckPosition = pos;
                unit.LastMovedTime = now;
                return;
            }

            bool targetMoving = target.Velocity.Length() > 3f;
            bool weAreStopped = unit.Vehicle.Speed < _cfg.StuckSpeedThreshold;
            bool tooFar = Utils.FlatDistance(pos, target.Position) > Math.Max(20f, unit.DistanceOffset + 10f);

            if (targetMoving && weAreStopped && tooFar && now - unit.LastMovedTime > _cfg.StuckTimeThreshold)
            {
                Vector3 recover = Utils.StreetNode(Utils.OffsetOf(target, new Vector3(0f, -unit.DistanceOffset - 4f, 0f)));
                unit.Vehicle.Position = recover;
                unit.Vehicle.Heading = target.Heading;
                unit.Vehicle.Velocity = Vector3.Zero;
                N.PlaceOnGroundProperly(unit.Vehicle.Handle);

                unit.LastCheckPosition = unit.Vehicle.Position;
                unit.LastMovedTime = now;
                unit.LastTaskTime = 0;
                Logger.Info("Konvoy araci sikisti, hedefin yanina isinlandi.");
            }
        }

        /// <summary>Konvoyun etrafinda toplanacagi hedef: oyuncunun araci > VIP araci > oyuncu.</summary>
        private Entity ResolveTarget(Ped player)
        {
            if (player.IsInVehicle())
            {
                Vehicle current = player.CurrentVehicle;
                if (Utils.Valid(current)) return current;
            }

            if (Utils.DrivableVehicle(_vipVehicle)) return _vipVehicle;
            return player;
        }

        private void UpdateUnit(ConvoyUnit unit, Entity target, Dictionary<int, Ped> assignment, int now)
        {
            if (!Utils.DrivableVehicle(unit.Vehicle)) return;

            // Surucu oldu mu? Ekipten birini direksiyona gecir.
            if (!Utils.AlivePed(unit.Driver))
            {
                if (!PromoteNewDriver(unit)) return;
                unit.LastTaskTime = 0;
            }

            // Ekip drive-by ile karsilik versin (paylastirilmis hedef atamasi kullanilir).
            if (now - unit.LastCombatTime > 4000)
            {
                Ped threat = null;
                if (assignment != null && Utils.AlivePed(unit.Driver))
                    assignment.TryGetValue(unit.Driver.Handle, out threat);

                if (Utils.AlivePed(threat))
                {
                    for (int i = 0; i < unit.Crew.Count; i++)
                    {
                        Ped crew = unit.Crew[i];
                        if (!Utils.AlivePed(crew)) continue;
                        N.TaskCombatPed(crew.Handle, threat.Handle);
                    }
                    unit.LastCombatTime = now;
                }
            }

            bool targetChanged = unit.LastTargetHandle != target.Handle;
            bool modeChanged = unit.LastMode != (int)SpeedMode;
            bool stale = now - unit.LastTaskTime > 3000;

            if (!targetChanged && !modeChanged && !stale) return;

            IssueDrivingTask(unit, target);
            unit.LastTargetHandle = target.Handle;
            unit.LastMode = (int)SpeedMode;
            unit.LastTaskTime = now;
        }

        private void IssueDrivingTask(ConvoyUnit unit, Entity target)
        {
            if (!Utils.AlivePed(unit.Driver) || !Utils.DrivableVehicle(unit.Vehicle)) return;

            GuardFactory.ConfigureDriver(unit.Driver, SpeedMode);

            float speed = CurrentSpeed;
            int style = CurrentDrivingStyle;

            Vehicle targetVehicle = target as Vehicle;
            if (targetVehicle != null && Utils.Valid(targetVehicle))
            {
                // Gercek eskort davranisi: hedefin onunde / arkasinda / yaninda kalir.
                N.TaskVehicleEscort(unit.Driver.Handle, unit.Vehicle.Handle, targetVehicle.Handle,
                    unit.Role, speed, style, Math.Max(_cfg.ConvoyMinDistance, unit.DistanceOffset), _cfg.ConvoyNoRoadsDistance);
            }
            else
            {
                // Oyuncu yayaysa: yavasca takip et.
                N.TaskVehicleFollow(unit.Driver.Handle, unit.Vehicle.Handle, target.Handle,
                    Math.Min(speed, 14f), style, (int)Math.Max(8f, unit.DistanceOffset));
            }
        }

        private bool PromoteNewDriver(ConvoyUnit unit)
        {
            for (int i = 0; i < unit.Crew.Count; i++)
            {
                Ped crew = unit.Crew[i];
                if (!Utils.AlivePed(crew)) continue;

                N.SetIntoVehicle(crew.Handle, unit.Vehicle.Handle, -1);
                GuardFactory.ConfigureDriver(crew, SpeedMode);
                unit.Driver = crew;
                unit.Crew.RemoveAt(i);
                Logger.Info("Konvoy: yeni surucu goreve gecti.");
                return true;
            }
            return false;
        }

        // ------------------------------------------------------------------
        // Durus / mevzilenme
        // ------------------------------------------------------------------
        private void UpdateHaltState(Entity target)
        {
            if (!_cfg.DeployOnStop) return;

            int now = Game.GameTime;
            float speed = 0f;

            Vehicle targetVehicle = target as Vehicle;
            if (targetVehicle != null && Utils.Valid(targetVehicle)) speed = targetVehicle.Speed;
            else if (Utils.Valid(target)) speed = target.Velocity.Length();

            if (speed < 1.5f)
            {
                _movingSince = 0;
                if (_stoppedSince == 0) _stoppedSince = now;

                if (_deployStage == 0 && now - _stoppedSince > 3000)
                    DeployCrew(target);
            }
            else
            {
                _stoppedSince = 0;
                if (_movingSince == 0) _movingSince = now;

                if (_deployStage > 0 && now - _movingSince > 1200)
                    RegroupCrew();
            }
        }

        /// <summary>Konvoy durdu: ekip inip 360 derece savunma alir.</summary>
        public void DeployCrew(Entity center)
        {
            if (!Active || _deployStage > 0) return;

            for (int i = 0; i < _units.Count; i++)
            {
                ConvoyUnit unit = _units[i];
                if (!Utils.Valid(unit.Vehicle)) continue;

                for (int j = 0; j < unit.Crew.Count; j++)
                {
                    Ped crew = unit.Crew[j];
                    if (!Utils.AlivePed(crew)) continue;

                    N.SetCombatAttribute(crew.Handle, 3, true);   // inmesine izin ver
                    N.TaskLeaveVehicle(crew.Handle, unit.Vehicle.Handle, 0);
                }
            }

            _deployStage = 1;
            _deployStageTime = Game.GameTime;
            Utils.Notify("Konvoy durdu - cevre guvenligi aliniyor.");
        }

        private void UpdateDeployedCrew(Entity center)
        {
            int now = Game.GameTime;

            // 1. asama: inis animasyonunun bitmesini bekle, sonra mevzi ver.
            if (_deployStage == 1 && now - _deployStageTime > 2500)
            {
                Vector3 origin = Utils.Valid(center) ? center.Position : Game.Player.Character.Position;

                List<Ped> deployed = CollectCrew();
                for (int i = 0; i < deployed.Count; i++)
                {
                    Ped crew = deployed[i];
                    if (!Utils.AlivePed(crew)) continue;

                    Vector3 spot = Utils.SafeGround(Utils.PointOnCircle(origin, _cfg.DeployRadius, i, deployed.Count));
                    N.ClearTasks(crew.Handle);
                    N.TaskStandGuard(crew.Handle, spot, Utils.HeadingTowards(origin, spot), "WORLD_HUMAN_GUARD_STAND");
                }

                _deployStage = 2;
                _deployStageTime = now;
            }

            // Mevzideyken tehdit gorurse engaje olsun (hedefler dagitilarak atanir).
            if (_deployStage == 2 && Behavior != GuardBehavior.Passive && _scanner.HasThreats &&
                now - _deployStageTime > 1500)
            {
                List<Ped> deployed = CollectCrew();
                Dictionary<int, Ped> assignment = _scanner.AssignTargets(deployed);

                for (int i = 0; i < deployed.Count; i++)
                {
                    Ped crew = deployed[i];
                    if (!Utils.AlivePed(crew)) continue;

                    Ped threat;
                    if (assignment.TryGetValue(crew.Handle, out threat) && Utils.AlivePed(threat))
                        N.TaskCombatPed(crew.Handle, threat.Handle);
                }

                _deployStageTime = now;   // tekrar hedef atamayi sinirla
            }
        }

        /// <summary>Ekip araclara geri biner.</summary>
        public void RegroupCrew()
        {
            if (_deployStage == 0) return;

            for (int i = 0; i < _units.Count; i++)
            {
                ConvoyUnit unit = _units[i];
                if (!Utils.DrivableVehicle(unit.Vehicle)) continue;

                for (int j = 0; j < unit.Crew.Count; j++)
                {
                    Ped crew = unit.Crew[j];
                    if (!Utils.AlivePed(crew)) continue;

                    int seat = j < N.GetMaxPassengers(unit.Vehicle.Handle) ? j : 0;
                    N.ClearTasks(crew.Handle);

                    if (crew.Position.DistanceTo(unit.Vehicle.Position) > 20f)
                        N.SetIntoVehicle(crew.Handle, unit.Vehicle.Handle, seat);
                    else
                        N.TaskEnterVehicle(crew.Handle, unit.Vehicle.Handle, 12000, seat, 2f, 1);

                    N.SetCombatAttribute(crew.Handle, 3, false);
                }

                unit.LastTaskTime = 0;
            }

            _deployStage = 0;
            _stoppedSince = 0;
        }

        private List<Ped> CollectCrew()
        {
            List<Ped> crew = new List<Ped>();
            for (int i = 0; i < _units.Count; i++)
            {
                for (int j = 0; j < _units[i].Crew.Count; j++)
                {
                    if (Utils.AlivePed(_units[i].Crew[j]))
                        crew.Add(_units[i].Crew[j]);
                }
            }
            return crew;
        }

        // ------------------------------------------------------------------
        // Ayarlar
        // ------------------------------------------------------------------
        public void SetSpeedMode(ConvoySpeedMode mode)
        {
            SpeedMode = mode;
            for (int i = 0; i < _units.Count; i++) _units[i].LastTaskTime = 0;
            Utils.Notify("Konvoy modu: " + SpeedModeName(mode));
        }

        public void SetFormation(Formation formation)
        {
            ConvoyFormation = formation;
            AssignRoles();
            for (int i = 0; i < _units.Count; i++) _units[i].LastTaskTime = 0;
            Utils.Notify("Konvoy dizilimi: " + CloseProtection.FormationName(formation));
        }

        public void CycleFormation()
        {
            Array values = Enum.GetValues(typeof(Formation));
            int index = Array.IndexOf(values, ConvoyFormation);
            SetFormation((Formation)values.GetValue((index + 1) % values.Length));
        }

        /// <summary>
        /// Secilen dizilime gore araclarin escort rollerini dagitir.
        ///
        /// NOT: Yan pozisyonlar icin kasitli olarak tam sol/sag (EscortMode.Left/Right)
        /// DEGIL, capraz arka (RearLeft/RearRight) kullanilir. Tam yan pozisyon normal
        /// iki seritli bir yolda araci fiziksel olarak karsi seride/kaldirima iter;
        /// bu da "konvoy virajda dagiliyor / trafige giriyor" sikayetinin ana
        /// sebeplerinden biridir. Capraz arka pozisyon ayni koruma hissini verirken
        /// araci kendi seridinde tutar.
        /// </summary>
        private void AssignRoles()
        {
            for (int i = 0; i < _units.Count; i++)
            {
                ConvoyUnit unit = _units[i];

                switch (ConvoyFormation)
                {
                    case Formation.Column:
                        unit.Role = i == 0 ? EscortMode.Front : EscortMode.Rear;
                        unit.DistanceOffset = _cfg.ConvoyMinDistance + i * 8f;
                        break;

                    case Formation.Box:
                    case Formation.Circle:
                        unit.Role = i % 4 == 0 ? EscortMode.Front
                                  : i % 4 == 1 ? EscortMode.Rear
                                  : i % 4 == 2 ? EscortMode.RearLeft : EscortMode.RearRight;
                        unit.DistanceOffset = _cfg.ConvoyMinDistance;
                        break;

                    case Formation.Wedge:
                        unit.Role = i % 2 == 0 ? EscortMode.RearLeft : EscortMode.RearRight;
                        unit.DistanceOffset = _cfg.ConvoyMinDistance + (i / 2) * 5f;
                        break;

                    default: // Diamond
                        unit.Role = i == 0 ? EscortMode.Front
                                  : i == 1 ? EscortMode.Rear
                                  : i == 2 ? EscortMode.RearLeft : EscortMode.RearRight;
                        unit.DistanceOffset = _cfg.ConvoyMinDistance + (i / 2) * 5f;
                        break;
                }
            }
        }

        /// <summary>VIP aracini degistirir (eski arac bizimse silinir).</summary>
        public void ChangeVipVehicle(string modelName)
        {
            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player)) return;

            bool playerWasInside = Utils.Valid(_vipVehicle) && player.IsInVehicle(_vipVehicle);

            Utils.RemoveBlip(_vipBlip);
            _vipBlip = null;

            if (_ownsVipVehicle) Utils.DeleteEntity(_vipVehicle);
            _vipVehicle = null;

            Vector3 spot = Utils.StreetNode(Utils.OffsetOf(player, new Vector3(0f, 6f, 0f)));
            _vipVehicle = GuardFactory.CreateVehicle(_cfg, modelName, spot, player.Heading);
            _ownsVipVehicle = Utils.Valid(_vipVehicle);

            if (!Utils.Valid(_vipVehicle))
            {
                Utils.Notify("VIP araci spawn edilemedi: " + modelName);
                return;
            }

            if (_cfg.EnableBlips)
                _vipBlip = Utils.AttachBlip(_vipVehicle, _cfg.ConvoyBlipSprite, _cfg.ConvoyBlipColor, "VIP Araci", 0.9f, false);

            if (playerWasInside || _cfg.WarpPlayerIntoVip)
                N.SetIntoVehicle(player.Handle, _vipVehicle.Handle, -1);

            for (int i = 0; i < _units.Count; i++) _units[i].LastTaskTime = 0;
            Utils.Notify("VIP araci: " + modelName);
        }

        /// <summary>Yakin korumanin binebilecegi, bos koltuklu bir konvoy araci dondurur.</summary>
        public Vehicle FindVehicleWithFreeSeat()
        {
            for (int i = 0; i < _units.Count; i++)
            {
                Vehicle vehicle = _units[i].Vehicle;
                if (!Utils.DrivableVehicle(vehicle)) continue;

                int maxPassengers = N.GetMaxPassengers(vehicle.Handle);
                for (int seat = 0; seat < maxPassengers; seat++)
                {
                    if (N.IsSeatFree(vehicle.Handle, seat)) return vehicle;
                }
            }
            return null;
        }

        // ------------------------------------------------------------------
        // Temizlik
        // ------------------------------------------------------------------
        private void PruneDeadUnits()
        {
            for (int i = _units.Count - 1; i >= 0; i--)
            {
                ConvoyUnit unit = _units[i];

                // Ekipteki oluleri temizle
                for (int j = unit.Crew.Count - 1; j >= 0; j--)
                {
                    if (!Utils.AlivePed(unit.Crew[j]))
                    {
                        if (_cfg.RemoveDeadBodies) Utils.DeleteEntity(unit.Crew[j]);
                        else Utils.Release(unit.Crew[j]);
                        unit.Crew.RemoveAt(j);
                    }
                }

                if (Utils.DrivableVehicle(unit.Vehicle)) continue;

                // Arac imha edildi: birimi kaldir.
                Utils.RemoveBlip(unit.Blip);
                if (Utils.Valid(unit.Driver))
                {
                    if (Utils.AlivePed(unit.Driver)) Utils.Release(unit.Driver);
                    else if (_cfg.RemoveDeadBodies) Utils.DeleteEntity(unit.Driver);
                }

                for (int j = 0; j < unit.Crew.Count; j++)
                    Utils.Release(unit.Crew[j]);

                if (Utils.Valid(unit.Vehicle)) Utils.Release(unit.Vehicle);
                _units.RemoveAt(i);
            }
        }

        public void Dismiss(bool silent = false)
        {
            for (int i = 0; i < _units.Count; i++)
            {
                ConvoyUnit unit = _units[i];
                Utils.RemoveBlip(unit.Blip);

                for (int j = 0; j < unit.Crew.Count; j++) Utils.DeleteEntity(unit.Crew[j]);
                Utils.DeleteEntity(unit.Driver);
                Utils.DeleteEntity(unit.Vehicle);
            }
            _units.Clear();

            Utils.RemoveBlip(_vipBlip);
            _vipBlip = null;

            if (_ownsVipVehicle && Utils.Valid(_vipVehicle))
            {
                Ped player = Game.Player.Character;
                bool playerInside = Utils.Valid(player) && player.IsInVehicle(_vipVehicle);

                // Oyuncu iceridiyse araci silme, sadece birak.
                if (playerInside) Utils.Release(_vipVehicle);
                else Utils.DeleteEntity(_vipVehicle);
            }

            _vipVehicle = null;
            _ownsVipVehicle = false;
            _deployStage = 0;
            _stoppedSince = 0;
            Active = false;

            if (!silent) Utils.Notify("Konvoy dagitildi.");
        }

        public void Toggle()
        {
            if (Active) Dismiss();
            else Create();
        }

        public static string SpeedModeName(ConvoySpeedMode mode)
        {
            switch (mode)
            {
                case ConvoySpeedMode.Aggressive: return "Agresif";
                case ConvoySpeedMode.Stealth: return "Sessiz";
                default: return "Normal";
            }
        }
    }
}
