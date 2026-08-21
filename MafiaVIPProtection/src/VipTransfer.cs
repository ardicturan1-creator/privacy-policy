using System;
using System.Collections.Generic;
using GTA;
using GTA.Math;

namespace MafiaVIP
{
    /// <summary>
    /// VIP Transfer — sinematik sofor hizmeti.
    ///
    /// Oyuncunun bulundugu konuma agir zirhli bir filo gelir, filodaki VIP araci
    /// oyuncunun kapisini acar, oyuncu arka koltuga biner, harita uzerinden
    /// isaretlenen (GPS waypoint) hedefe goturulur ve orada dikkatlice indirilir.
    ///
    /// Bu sinif TAMAMEN BAGIMSIZDIR: CloseProtection/ConvoyManager/AirSupport/
    /// BackupManager'in hicbir alanina dokunmaz, kendi arac/ped listesini kendi
    /// yonetir. Boylece mevcut sistemler bu ozellik eklenirken bozulmaz.
    ///
    /// Harita secimi icin oyunun kendi duraklat menusu/haritasi kullanilir (bu,
    /// oyunun her surumunde calismasi garanti tek yontemdir): oyuncu haritayi
    /// acip normal sekilde bir GPS hedefi isaretler, script bunu
    /// World.WaypointPosition ile okur. Boylece riskli/belgelenmemis "haritayi
    /// programatik olarak ac" native'lerine bagimli olunmaz.
    ///
    /// Guvenilirlik ilkesi: HER bekleme asamasinin bir zaman asimi ve garantili
    /// bir yedek (fallback) yolu vardir - AI gorevi basarisiz olsa bile islem
    /// sonsuza kadar takilip kalmaz, gerekirse dogrudan isinlanarak tamamlanir.
    /// </summary>
    internal sealed class VipTransfer
    {
        // ------------------------------------------------------------------
        // Asamalar
        // ------------------------------------------------------------------
        private const int StageIdle = 0;
        private const int StageDriveToPlayer = 1;
        private const int StageDoorOpenPickup = 2;
        private const int StageBoarding = 3;
        private const int StageDoorClosePickup = 4;
        private const int StageAwaitWaypoint = 5;
        private const int StageEnRoute = 6;
        private const int StageDoorOpenDropoff = 7;
        private const int StageExiting = 8;
        private const int StageDoorCloseDropoff = 9;
        private const int StageDeparting = 10;

        private sealed class TransferVehicle
        {
            public Vehicle Vehicle;
            public Ped Driver;
            public readonly List<Ped> Guards = new List<Ped>();
            public Blip Blip;
            public int Role;
            public int LastTaskTime;
            public Vector3 LastCheckPosition;
            public int LastMovedTime;
        }

        private readonly Config _cfg;
        private readonly List<TransferVehicle> _vehicles = new List<TransferVehicle>();

        private int _stage;
        private int _stageTime;
        private int _stageStartTime;      // asamanin genel zaman asimi icin sabit baslangic
        private bool _taskIssued;         // tek seferlik gorev bayragi (her tick yeniden tetiklenmesin)
        private Vector3 _pickupPoint;
        private Vector3 _destination;
        private VehicleDoorIndex _activeDoor;
        private int _boardSeat;

        public VipTransfer(Config cfg)
        {
            _cfg = cfg;
        }

        public bool Active { get { return _stage != StageIdle; } }

        /// <summary>Menu/HUD icin kisa durum metni.</summary>
        public string StatusText
        {
            get
            {
                switch (_stage)
                {
                    case StageIdle: return "Kapali";
                    case StageDriveToPlayer: return "Konvoy yaklasiyor";
                    case StageDoorOpenPickup: return "Kapi aciliyor";
                    case StageBoarding: return "Binis";
                    case StageDoorClosePickup: return "Kapi kapaniyor";
                    case StageAwaitWaypoint: return "Hedef bekleniyor";
                    case StageEnRoute: return "Yolda";
                    case StageDoorOpenDropoff: return "Varildi - kapi aciliyor";
                    case StageExiting: return "Inis";
                    case StageDoorCloseDropoff: return "Kapi kapaniyor";
                    case StageDeparting: return "Konvoy ayriliyor";
                    default: return "?";
                }
            }
        }

        // ==================================================================
        // Baslatma
        // ==================================================================
        public void Start()
        {
            if (_stage != StageIdle)
            {
                Utils.Notify("VIP transfer zaten devam ediyor.");
                return;
            }

            Ped player = Game.Player.Character;
            if (!Utils.AlivePed(player))
            {
                Utils.Notify("VIP transfer su anda baslatilamaz.");
                return;
            }

            if (player.IsInVehicle())
            {
                Utils.Notify("VIP transfer icin once araçtan inin.");
                return;
            }

            try
            {
                CleanupAll();   // savunma amacli: olasi artik varliklari temizle

                _pickupPoint = player.Position;
                int total = _cfg.VipTransferVehicleCount;

                for (int i = 0; i < total; i++)
                {
                    float angle = i * (360f / Math.Max(total, 1)) + 25f;
                    Vector3 spawnDir = new Vector3(
                        (float)Math.Sin(angle * Math.PI / 180.0),
                        (float)Math.Cos(angle * Math.PI / 180.0), 0f);
                    Vector3 spawnPoint = Utils.StreetNode(player.Position + spawnDir * (35f + i * 6f));

                    bool isVip = i == 0;
                    Vehicle vehicle = GuardFactory.CreateVehicle(_cfg, _cfg.VipTransferVehicleModels, spawnPoint,
                        Utils.HeadingTowards(spawnPoint, player.Position));
                    if (!Utils.Valid(vehicle)) continue;

                    Ped driver = GuardFactory.CreateGuard(_cfg, spawnPoint, vehicle.Heading);
                    if (driver == null)
                    {
                        Utils.DeleteEntity(vehicle);
                        continue;
                    }

                    N.SetIntoVehicle(driver.Handle, vehicle.Handle, -1);
                    GuardFactory.ConfigureDriver(driver, ConvoySpeedMode.Normal);

                    TransferVehicle tv = new TransferVehicle { Vehicle = vehicle, Driver = driver };

                    if (isVip)
                    {
                        // VIP araca on koltuga bir koruma binidiriliyor: hem tema
                        // (sofor + korumali VIP araci) hem de guvenlik agi - sofor
                        // vurulursa devralacak biri olmadan transfer sonsuza kadar
                        // beklemesin diye. Oyuncu arka koltuklardan birine binecegi
                        // icin on koltuk (indeks 0) her zaman bostur.
                        Ped bodyguard = GuardFactory.CreateGuard(_cfg, spawnPoint, vehicle.Heading);
                        if (bodyguard != null)
                        {
                            N.SetIntoVehicle(bodyguard.Handle, vehicle.Handle, 0);
                            N.SetCombatAttribute(bodyguard.Handle, 2, true);
                            N.SetCombatAttribute(bodyguard.Handle, 3, false);
                            tv.Guards.Add(bodyguard);
                        }
                    }
                    else
                    {
                        int maxPassengers = N.GetMaxPassengers(vehicle.Handle);
                        int guardCount = Math.Min(_cfg.VipTransferGuardsPerEscort, maxPassengers);
                        for (int seat = 0; seat < guardCount; seat++)
                        {
                            Ped guard = GuardFactory.CreateGuard(_cfg, spawnPoint, vehicle.Heading);
                            if (guard == null) break;

                            N.SetIntoVehicle(guard.Handle, vehicle.Handle, seat);
                            N.SetCombatAttribute(guard.Handle, 2, true);
                            N.SetCombatAttribute(guard.Handle, 3, false);
                            tv.Guards.Add(guard);
                        }

                        // Escort rolleri: on / arka / capraz-arka (bkz. EscortMode dokumantasyonu).
                        int escortIndex = i - 1;
                        tv.Role = escortIndex == 0 ? EscortMode.Front
                                : escortIndex == 1 ? EscortMode.Rear
                                : escortIndex == 2 ? EscortMode.RearLeft
                                : EscortMode.RearRight;
                    }

                    if (_cfg.EnableBlips)
                    {
                        tv.Blip = Utils.AttachBlip(vehicle, _cfg.ConvoyBlipSprite, _cfg.ConvoyBlipColor,
                            isVip ? "VIP Transfer Araci" : "VIP Transfer Escort", 0.8f);
                    }

                    _vehicles.Add(tv);
                }

                if (_vehicles.Count == 0)
                {
                    Utils.Notify("VIP transfer araclari spawn edilemedi. Model adini kontrol edin.");
                    return;
                }

                IssueApproach();

                _stage = StageDriveToPlayer;
                _stageTime = Game.GameTime;
                _stageStartTime = Game.GameTime;

                Utils.Notify(string.Format("VIP transfer konvoyu yola cikti ({0} arac).", _vehicles.Count));
                Logger.Info("VipTransfer baslatildi: " + _vehicles.Count + " arac.");
            }
            catch (Exception ex)
            {
                Logger.Error("VipTransfer.Start hatasi", ex);
                CleanupAll();
            }
        }

        /// <summary>Herhangi bir asamada guvenle iptal eder.</summary>
        public void Cancel()
        {
            if (_stage == StageIdle)
            {
                Utils.Notify("Aktif VIP transfer yok.");
                return;
            }

            try
            {
                Ped player = Game.Player.Character;
                TransferVehicle vip = _vehicles.Count > 0 ? _vehicles[0] : null;

                // Oyuncu aractaysa once guvenli bir sekilde disari cikar.
                if (Utils.Valid(player) && vip != null && Utils.Valid(vip.Vehicle) && player.IsInVehicle(vip.Vehicle))
                {
                    OpenNearestDoor(vip.Vehicle, player.Position, out _activeDoor);
                    Vector3 safeSpot = Utils.SafeGround(Utils.OffsetOf(vip.Vehicle, new Vector3(2.5f, 0f, 0f)));
                    player.Position = safeSpot;
                    N.ClearTasksImmediately(player.Handle);
                }

                Utils.Notify("VIP transfer iptal edildi.");
                Logger.Info("VipTransfer iptal edildi (asama " + _stage + ").");
            }
            catch (Exception ex)
            {
                Logger.Error("VipTransfer.Cancel hatasi", ex);
            }
            finally
            {
                CleanupAll();
            }
        }

        /// <summary>Menudeki "Hedefi Onayla" butonu bunu cagirir.</summary>
        public void ConfirmDestination()
        {
            if (_stage != StageAwaitWaypoint)
            {
                Utils.Notify("Su anda hedef onayi beklenmiyor.");
                return;
            }

            if (World.WaypointBlip == null)
            {
                Utils.Notify("Once haritayi acip bir hedef isaretleyin (GPS waypoint).");
                return;
            }

            _destination = World.WaypointPosition;
            World.RemoveWaypoint();

            for (int i = 0; i < _vehicles.Count; i++) _vehicles[i].LastTaskTime = 0;

            IssueTravel();

            _stage = StageEnRoute;
            _stageTime = Game.GameTime;
            _stageStartTime = Game.GameTime;

            Utils.Notify("Hedef alindi, hareket ediliyor.");
        }

        // ==================================================================
        // Ana dongu
        // ==================================================================
        public void Update()
        {
            if (_stage == StageIdle) return;

            try
            {
                Ped player = Game.Player.Character;
                if (!Utils.Valid(player))
                {
                    CleanupAll();
                    return;
                }

                // Konvoydaki tum araclarin surucusu olurse yenisini goreve al.
                for (int i = 0; i < _vehicles.Count; i++)
                    EnsureDriver(_vehicles[i]);

                // Oyuncu bindikten sonra (bekleme veya yolculuk asamasinda) kendi
                // istegiyle araçtan cikarsa konvoy bunu fark etmeden hedefe kadar
                // surmeye devam etmesin - transferi guvenle iptal et.
                if ((_stage == StageAwaitWaypoint || _stage == StageEnRoute) && !PlayerStillAboard(player))
                {
                    Utils.Notify("Araçtan ayrildiniz, VIP transfer iptal edildi.");
                    CleanupAll();
                    return;
                }

                switch (_stage)
                {
                    case StageDriveToPlayer: UpdateDriveToPlayer(player); break;
                    case StageDoorOpenPickup: UpdateDoorOpenPickup(player); break;
                    case StageBoarding: UpdateBoarding(player); break;
                    case StageDoorClosePickup: UpdateDoorClosePickup(player); break;
                    case StageAwaitWaypoint: /* ConfirmDestination() menuden cagrilir */ break;
                    case StageEnRoute: UpdateEnRoute(player); break;
                    case StageDoorOpenDropoff: UpdateDoorOpenDropoff(player); break;
                    case StageExiting: UpdateExiting(player); break;
                    case StageDoorCloseDropoff: UpdateDoorCloseDropoff(player); break;
                    case StageDeparting: UpdateDeparting(player); break;
                }
            }
            catch (Exception ex)
            {
                Logger.Error("VipTransfer.Update hatasi (asama " + _stage + ")", ex);
                CleanupAll();
            }
        }

        // ------------------------------------------------------------------
        // Alis (pickup)
        // ------------------------------------------------------------------
        private void UpdateDriveToPlayer(Ped player)
        {
            TransferVehicle vip = _vehicles[0];
            if (!Utils.DrivableVehicle(vip.Vehicle))
            {
                Utils.Notify("VIP transfer araci imha edildi, iptal ediliyor.");
                CleanupAll();
                return;
            }

            int now = Game.GameTime;

            if (now - _stageStartTime > _cfg.VipTransferStageTimeoutPickup)
            {
                // Guvenli yedek: araclari zorla oyuncunun yanina getir.
                Logger.Info("VipTransfer: alis zaman asimi, araclar isinlaniyor.");
                for (int i = 0; i < _vehicles.Count; i++)
                {
                    TransferVehicle tv = _vehicles[i];
                    if (!Utils.DrivableVehicle(tv.Vehicle)) continue;

                    Vector3 spot = Utils.PointOnCircle(player.Position, 6f + i * 2f, i, _vehicles.Count);
                    tv.Vehicle.Position = Utils.SafeGround(spot);
                    tv.Vehicle.Velocity = Vector3.Zero;
                    N.PlaceOnGroundProperly(tv.Vehicle.Handle);
                }
            }
            else
            {
                if (now - vip.LastTaskTime > 3000) IssueApproach();
                CheckStuckAndRecover(player.Position);
            }

            float distance = vip.Vehicle.Position.DistanceTo(player.Position);
            if (distance <= _cfg.VipTransferArrivalDistance && vip.Vehicle.Speed < _cfg.StuckSpeedThreshold + 1f)
            {
                Utils.Notify("Konvoy geldi.");
                if (Utils.AlivePed(vip.Driver))
                    N.TaskLeaveVehicle(vip.Driver.Handle, vip.Vehicle.Handle, 0);

                _stage = StageDoorOpenPickup;
                _stageTime = now;
                _taskIssued = false;
            }
        }

        private void UpdateDoorOpenPickup(Ped player)
        {
            int now = Game.GameTime;
            if (now - _stageTime < _cfg.VipTransferDoorDelay) return;

            TransferVehicle vip = _vehicles[0];
            if (!Utils.DrivableVehicle(vip.Vehicle)) { CleanupAll(); return; }

            _boardSeat = OpenNearestDoor(vip.Vehicle, player.Position, out _activeDoor);
            Utils.Notify("Kapiniz aciliyor...");

            _stage = StageBoarding;
            _stageTime = now;
            _taskIssued = false;
        }

        private void UpdateBoarding(Ped player)
        {
            TransferVehicle vip = _vehicles[0];
            if (!Utils.DrivableVehicle(vip.Vehicle)) { CleanupAll(); return; }

            if (Utils.Valid(player) && player.IsInVehicle(vip.Vehicle))
            {
                Utils.Notify("Arka koltuga bindiniz.");
                _stage = StageDoorClosePickup;
                _stageTime = Game.GameTime;
                _taskIssued = false;
                return;
            }

            int now = Game.GameTime;

            if (!_taskIssued)
            {
                N.TaskEnterVehicle(player.Handle, vip.Vehicle.Handle, _cfg.VipTransferBoardTimeout, _boardSeat, 1.5f, 1);
                _taskIssued = true;
            }
            else if (now - _stageTime > _cfg.VipTransferBoardTimeout)
            {
                // Guvenli yedek: dogrudan koltuga isinla.
                N.ClearTasksImmediately(player.Handle);
                N.SetIntoVehicle(player.Handle, vip.Vehicle.Handle, _boardSeat);
                Logger.Info("VipTransfer: binis zaman asimi, oyuncu koltuga isinlandi.");
            }
        }

        private void UpdateDoorClosePickup(Ped player)
        {
            int now = Game.GameTime;
            if (now - _stageTime < _cfg.VipTransferDoorDelay) return;

            TransferVehicle vip = _vehicles[0];
            if (Utils.Valid(vip.Vehicle))
            {
                vip.Vehicle.Doors[_activeDoor].Close(false);
                Utils.Notify("Kapiniz kapandi.");
            }

            if (Utils.AlivePed(vip.Driver) && Utils.Valid(vip.Vehicle) && !vip.Driver.IsInVehicle(vip.Vehicle))
                N.TaskEnterVehicle(vip.Driver.Handle, vip.Vehicle.Handle, _cfg.VipTransferBoardTimeout, -1, 1.5f, 1);

            _stage = StageAwaitWaypoint;
            _stageTime = now;
            _taskIssued = false;

            Utils.Notify("Haritayi acip hedefi isaretleyin, sonra VIP Transfer menusunden 'Hedefi Onayla'ya basin.");
            Utils.Subtitle("Hedef bekleniyor - haritadan bir GPS noktasi isaretleyin.", 4000);
        }

        // ------------------------------------------------------------------
        // Yol (en route) — surucu zorla oturtma yedek kontrolu de burada.
        // ------------------------------------------------------------------
        private void UpdateEnRoute(Ped player)
        {
            TransferVehicle vip = _vehicles[0];

            // Surucu hala koltuga oturmadiysa (StageDoorClosePickup'ta task verildi) yedek isinlama.
            if (Utils.AlivePed(vip.Driver) && Utils.Valid(vip.Vehicle) && !vip.Driver.IsInVehicle(vip.Vehicle) &&
                Game.GameTime - _stageTime > _cfg.VipTransferBoardTimeout)
            {
                N.SetIntoVehicle(vip.Driver.Handle, vip.Vehicle.Handle, -1);
            }

            if (!Utils.DrivableVehicle(vip.Vehicle))
            {
                Utils.Notify("VIP araci imha edildi, transfer iptal ediliyor.");
                CleanupAll();
                return;
            }

            int now = Game.GameTime;

            if (now - _stageStartTime > _cfg.VipTransferStageTimeoutTravel)
            {
                Utils.Notify("Hedefe ulasilamadi, mevcut konumda durduruluyor.");
                Logger.Info("VipTransfer: seyahat zaman asimi, mevcut konumda birakiliyor.");
            }
            else
            {
                if (now - vip.LastTaskTime > 3000) IssueTravel();
                CheckStuckAndRecover(_destination);

                float remaining = vip.Vehicle.Position.DistanceTo(_destination);
                if (remaining > _cfg.VipTransferArrivalDistance || vip.Vehicle.Speed >= _cfg.StuckSpeedThreshold + 1f)
                    return;
            }

            Utils.Notify("Hedefe varildi.");
            if (Utils.AlivePed(vip.Driver))
                N.TaskLeaveVehicle(vip.Driver.Handle, vip.Vehicle.Handle, 0);

            _stage = StageDoorOpenDropoff;
            _stageTime = now;
            _taskIssued = false;
        }

        // ------------------------------------------------------------------
        // Inis (dropoff)
        // ------------------------------------------------------------------
        private void UpdateDoorOpenDropoff(Ped player)
        {
            int now = Game.GameTime;
            if (now - _stageTime < _cfg.VipTransferDoorDelay) return;

            TransferVehicle vip = _vehicles[0];
            if (!Utils.Valid(vip.Vehicle)) { CleanupAll(); return; }

            // Oyuncu bindigi koltukta oturuyor; ayni tarafin kapisini tekrar acmak
            // (bosta bir "en yakin kapi" hesabi yerine) hem daha dogru hem daha basit.
            _activeDoor = _boardSeat == 2 ? VehicleDoorIndex.BackRightDoor : VehicleDoorIndex.BackLeftDoor;
            vip.Vehicle.Doors[_activeDoor].Open(false, false);

            Utils.Notify("Kapiniz aciliyor...");
            _stage = StageExiting;
            _stageTime = now;
            _taskIssued = false;
        }

        private void UpdateExiting(Ped player)
        {
            TransferVehicle vip = _vehicles[0];
            if (!Utils.Valid(vip.Vehicle)) { CleanupAll(); return; }

            if (!player.IsInVehicle(vip.Vehicle))
            {
                Utils.Notify("Iyi gunler patron, vardiniz.");
                _stage = StageDoorCloseDropoff;
                _stageTime = Game.GameTime;
                _taskIssued = false;
                return;
            }

            int now = Game.GameTime;

            if (!_taskIssued)
            {
                N.TaskLeaveVehicle(player.Handle, vip.Vehicle.Handle, 0);
                _taskIssued = true;
            }
            else if (now - _stageTime > _cfg.VipTransferBoardTimeout)
            {
                // Guvenli yedek: aracin yanina dogrudan isinla.
                Vector3 safeSpot = Utils.SafeGround(Utils.OffsetOf(vip.Vehicle, new Vector3(2.5f, 0f, 0f)));
                player.Position = safeSpot;
                N.ClearTasksImmediately(player.Handle);
                Logger.Info("VipTransfer: inis zaman asimi, oyuncu disariya isinlandi.");
            }
        }

        private void UpdateDoorCloseDropoff(Ped player)
        {
            int now = Game.GameTime;
            if (now - _stageTime < _cfg.VipTransferDoorDelay) return;

            TransferVehicle vip = _vehicles[0];
            if (Utils.Valid(vip.Vehicle)) vip.Vehicle.Doors[_activeDoor].Close(false);

            if (Utils.AlivePed(vip.Driver) && Utils.Valid(vip.Vehicle) && !vip.Driver.IsInVehicle(vip.Vehicle))
                N.TaskEnterVehicle(vip.Driver.Handle, vip.Vehicle.Handle, _cfg.VipTransferBoardTimeout, -1, 1.5f, 1);

            IssueDepart(player);

            _stage = StageDeparting;
            _stageTime = now;
        }

        private void UpdateDeparting(Ped player)
        {
            int now = Game.GameTime;

            bool allFarOrTimedOut = true;
            for (int i = 0; i < _vehicles.Count; i++)
            {
                TransferVehicle tv = _vehicles[i];
                if (!Utils.Valid(tv.Vehicle)) continue;
                if (tv.Vehicle.Position.DistanceTo(player.Position) < _cfg.VipTransferDepartDistance)
                    allFarOrTimedOut = false;
            }

            if (allFarOrTimedOut || now - _stageTime > 40000)
                CleanupAll();
        }

        // ==================================================================
        // Surus gorevleri
        // ==================================================================
        private void IssueApproach()
        {
            IssueDrivingTasks(Utils.StreetNode(_pickupPoint), _cfg.SpeedNormal, _cfg.DrivingStyleNormal, false);
        }

        private void IssueTravel()
        {
            IssueDrivingTasks(_destination, _cfg.SpeedAggressive, _cfg.DrivingStyleAggressive, true);
        }

        private void IssueDrivingTasks(Vector3 target, float speed, int drivingStyle, bool longRange)
        {
            int now = Game.GameTime;
            TransferVehicle vip = _vehicles[0];

            if (Utils.AlivePed(vip.Driver) && Utils.DrivableVehicle(vip.Vehicle))
            {
                GuardFactory.ConfigureDriver(vip.Driver, longRange ? ConvoySpeedMode.Aggressive : ConvoySpeedMode.Normal);
                N.TaskVehicleDriveToCoord(vip.Driver.Handle, vip.Vehicle.Handle, target, speed, drivingStyle, 8f);
                vip.LastTaskTime = now;
            }

            for (int i = 1; i < _vehicles.Count; i++)
            {
                TransferVehicle tv = _vehicles[i];
                if (!Utils.AlivePed(tv.Driver) || !Utils.DrivableVehicle(tv.Vehicle) || !Utils.DrivableVehicle(vip.Vehicle))
                    continue;

                GuardFactory.ConfigureDriver(tv.Driver, longRange ? ConvoySpeedMode.Aggressive : ConvoySpeedMode.Normal);
                N.TaskVehicleEscort(tv.Driver.Handle, tv.Vehicle.Handle, vip.Vehicle.Handle,
                    tv.Role, speed, drivingStyle, _cfg.ConvoyMinDistance, _cfg.ConvoyNoRoadsDistance);
                tv.LastTaskTime = now;
            }
        }

        private void IssueDepart(Ped player)
        {
            Vector3 away = player.Position + new Vector3(200f, 200f, 0f);
            for (int i = 0; i < _vehicles.Count; i++)
            {
                TransferVehicle tv = _vehicles[i];
                if (!Utils.AlivePed(tv.Driver) || !Utils.DrivableVehicle(tv.Vehicle)) continue;

                N.TaskVehicleDriveToCoord(tv.Driver.Handle, tv.Vehicle.Handle,
                    Utils.StreetNode(away + Utils.PointOnCircle(Vector3.Zero, 15f, i, _vehicles.Count)),
                    _cfg.SpeedNormal, _cfg.DrivingStyleNormal, 20f);
            }
        }

        // ------------------------------------------------------------------
        // Kapi / koltuk yardimcilari
        // ------------------------------------------------------------------
        /// <summary>
        /// Oyuncuya en yakin arka kapiyi acar ve karsilik gelen koltuk indeksini dondurur
        /// (1 = sol arka, 2 = sag arka - standart GTA V koltuk numaralandirmasi).
        /// </summary>
        private int OpenNearestDoor(Vehicle vehicle, Vector3 referencePosition, out VehicleDoorIndex door)
        {
            Vector3 toReference = referencePosition - vehicle.Position;
            float side = vehicle.RightVector.X * toReference.X + vehicle.RightVector.Y * toReference.Y;

            if (side >= 0f)
            {
                door = VehicleDoorIndex.BackRightDoor;
                vehicle.Doors[door].Open(false, false);
                return 2;
            }

            door = VehicleDoorIndex.BackLeftDoor;
            vehicle.Doors[door].Open(false, false);
            return 1;
        }

        /// <summary>Oyuncu binis sonrasi hala VIP araçta mi? (kendi istegiyle inebilir.)</summary>
        private bool PlayerStillAboard(Ped player)
        {
            TransferVehicle vip = _vehicles.Count > 0 ? _vehicles[0] : null;
            if (vip == null || !Utils.Valid(vip.Vehicle)) return false;
            return player.IsInVehicle(vip.Vehicle);
        }

        private void EnsureDriver(TransferVehicle tv)
        {
            if (Utils.AlivePed(tv.Driver) || !Utils.DrivableVehicle(tv.Vehicle)) return;

            for (int i = 0; i < tv.Guards.Count; i++)
            {
                Ped candidate = tv.Guards[i];
                if (!Utils.AlivePed(candidate)) continue;

                N.SetIntoVehicle(candidate.Handle, tv.Vehicle.Handle, -1);
                tv.Driver = candidate;
                tv.Guards.RemoveAt(i);
                tv.LastTaskTime = 0;
                return;
            }
        }

        /// <summary>Konvoy araci uzun sure ilerlemiyorsa hedefin yanina isinlayarak kurtarir.</summary>
        private void CheckStuckAndRecover(Vector3 target)
        {
            int now = Game.GameTime;

            for (int i = 0; i < _vehicles.Count; i++)
            {
                TransferVehicle tv = _vehicles[i];
                if (!Utils.DrivableVehicle(tv.Vehicle) || !Utils.AlivePed(tv.Driver)) continue;

                Vector3 pos = tv.Vehicle.Position;

                if (tv.LastMovedTime == 0)
                {
                    tv.LastCheckPosition = pos;
                    tv.LastMovedTime = now;
                    continue;
                }

                if (Utils.FlatDistance(pos, tv.LastCheckPosition) > 3f)
                {
                    tv.LastCheckPosition = pos;
                    tv.LastMovedTime = now;
                    continue;
                }

                bool weAreStopped = tv.Vehicle.Speed < _cfg.StuckSpeedThreshold;
                bool tooFar = Utils.FlatDistance(pos, target) > _cfg.VipTransferArrivalDistance + 15f;

                if (weAreStopped && tooFar && now - tv.LastMovedTime > _cfg.StuckTimeThreshold)
                {
                    Vector3 recover = Utils.StreetNode(target + Utils.PointOnCircle(Vector3.Zero, 10f + i * 3f, i, _vehicles.Count));
                    tv.Vehicle.Position = recover;
                    tv.Vehicle.Velocity = Vector3.Zero;
                    N.PlaceOnGroundProperly(tv.Vehicle.Handle);

                    tv.LastCheckPosition = tv.Vehicle.Position;
                    tv.LastMovedTime = now;
                    tv.LastTaskTime = 0;
                    Logger.Info("VipTransfer araci sikisti, hedefin yanina isinlandi.");
                }
            }
        }

        // ------------------------------------------------------------------
        // Temizlik
        // ------------------------------------------------------------------
        /// <summary>
        /// Tum transfer varliklarini siler. Cagiran taraf ne olursa olsun
        /// (iptal, "Tum Ekipleri Temizle", ayar yeniden yukleme, script kapanisi)
        /// GUVENLIDIR: oyuncu su an silinecek araclardan birinin icindeyse
        /// once disari cikarilir - aksi halde arac altindan kaybolur/dusulur.
        /// </summary>
        public void CleanupAll()
        {
            Ped player = Game.Player.Character;
            if (Utils.Valid(player))
            {
                for (int i = 0; i < _vehicles.Count; i++)
                {
                    Vehicle vehicle = _vehicles[i].Vehicle;
                    if (!Utils.Valid(vehicle) || !player.IsInVehicle(vehicle)) continue;

                    Vector3 safeSpot = Utils.SafeGround(Utils.OffsetOf(vehicle, new Vector3(2.5f, 0f, 0f)));
                    player.Position = safeSpot;
                    N.ClearTasksImmediately(player.Handle);
                    break;
                }
            }

            for (int i = 0; i < _vehicles.Count; i++)
            {
                TransferVehicle tv = _vehicles[i];
                Utils.RemoveBlip(tv.Blip);

                for (int j = 0; j < tv.Guards.Count; j++) Utils.DeleteEntity(tv.Guards[j]);
                Utils.DeleteEntity(tv.Driver);
                Utils.DeleteEntity(tv.Vehicle);
            }
            _vehicles.Clear();

            _stage = StageIdle;
            _stageTime = 0;
            _stageStartTime = 0;
            _taskIssued = false;
        }
    }
}
