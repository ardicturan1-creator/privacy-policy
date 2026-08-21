using GTA;
using GTA.Math;
using GTA.Native;

namespace MafiaVIP
{
    /// <summary>
    /// Native cagrilari icin ince bir sarmalayici.
    /// Hash degerleri HAM (raw) olarak yazildi ve (Hash) cast'i ile cagriliyor.
    /// Boylece farkli ScriptHookVDotNet v3 surumlerindeki "Hash" enum isim
    /// degisikliklerinden (ornegin _SET_X -> SET_X) etkilenmiyoruz; kod her
    /// SHVDN v3 buildinde derlenir ve calisir.
    ///
    /// Tum fonksiyonlar entity HANDLE'i (int) alir; boylece InputArgument
    /// implicit cast farkliliklari da devre disi kalir.
    /// </summary>
    internal static class N
    {
        // ------------------------------------------------------------------
        // Cekirdek cagri sarmalayicilari
        // ------------------------------------------------------------------
        public static void Call(ulong hash, params InputArgument[] args)
        {
            Function.Call((Hash)hash, args);
        }

        public static T Call<T>(ulong hash, params InputArgument[] args)
        {
            return Function.Call<T>((Hash)hash, args);
        }

        // ------------------------------------------------------------------
        // Hash sabitleri
        // ------------------------------------------------------------------
        private const ulong H_SET_PED_ARMOUR = 0xCEA04D83135264CC;
        private const ulong H_SET_PED_MAX_HEALTH = 0x166E7CF68597D8B5;
        private const ulong H_SET_ENTITY_HEALTH = 0x6B76DC1F3AE6E6A3;
        private const ulong H_SET_PED_ACCURACY = 0x7AEFB85C1D49DEB6;
        private const ulong H_SET_PED_COMBAT_ATTRIBUTES = 0x9F7794730795E019;
        private const ulong H_SET_PED_COMBAT_ABILITY = 0xC7622C0D36B2FDA8;
        private const ulong H_SET_PED_COMBAT_MOVEMENT = 0x4D9CA1009AFBD057;
        private const ulong H_SET_PED_COMBAT_RANGE = 0x3C606747B23E497B;
        private const ulong H_SET_PED_SEEING_RANGE = 0xF29CF591C4BF6CEE;
        private const ulong H_SET_PED_HEARING_RANGE = 0x33A8F7F7D5F7F33C;
        private const ulong H_SET_PED_ALERTNESS = 0xDBA71115ED9941A6;
        private const ulong H_SET_PED_SHOOT_RATE = 0x614DA022990752DC;
        private const ulong H_SET_PED_FIRING_PATTERN = 0x9AC577F5A12AD8A9;
        private const ulong H_SET_PED_CAN_SWITCH_WEAPON = 0xED7F7EFE9FABF340;
        private const ulong H_SET_PED_KEEP_TASK = 0x971D38760FBC02EF;
        private const ulong H_SET_PED_SUFFERS_CRITICAL_HITS = 0xEBD76F2359F190AC;
        private const ulong H_SET_PED_DIES_WHEN_INJURED = 0x5BA7919BED300023;
        private const ulong H_SET_PED_CAN_RAGDOLL = 0xB128377056A54E2A;
        private const ulong H_SET_PED_CAN_BE_TARGETTED = 0x63F58F7C80513AAD;
        private const ulong H_SET_PED_CONFIG_FLAG = 0x1913FE4CBF41C463;
        private const ulong H_SET_PED_RELATIONSHIP_GROUP_HASH = 0xC80A74AC829DDD92;
        private const ulong H_GET_PED_RELATIONSHIP_GROUP_HASH = 0x7DBDD04862D95F04;
        private const ulong H_SET_CAN_ATTACK_FRIENDLY = 0xB3B1CB349FF9C75D;
        private const ulong H_SET_PED_INFINITE_AMMO = 0x3EDCB0505123623B;
        private const ulong H_SET_PED_INFINITE_AMMO_CLIP = 0x183DADC6AA953186;
        private const ulong H_SET_CURRENT_PED_WEAPON = 0xADF692B254977C0C;
        private const ulong H_SET_PED_CAN_BE_KNOCKED_OFF_VEHICLE = 0x7A6535691B477C48;
        private const ulong H_SET_PED_RANDOM_COMPONENT_VARIATION = 0xC8A9481A01E63C28;
        private const ulong H_SET_PED_COMPONENT_VARIATION = 0x262B14F48D29DE80;
        private const ulong H_SET_PED_INTO_VEHICLE = 0xF75B0D629E1C063D;
        private const ulong H_SET_PED_HELMET = 0x560A43136EB58105;
        private const ulong H_SET_DRIVER_ABILITY = 0xB195FFA8042FC5C3;
        private const ulong H_SET_DRIVER_AGGRESSIVENESS = 0xA731F608CA104E3C;

        private const ulong H_IS_PED_IN_COMBAT = 0x4859F1FC66A6278E;
        private const ulong H_IS_PED_SHOOTING = 0x34616828CD07F1A1;
        private const ulong H_GET_PED_TYPE = 0xFF059E1E4C01E63C;
        private const ulong H_HAS_ENTITY_BEEN_DAMAGED_BY_ENTITY = 0xC86D67D52A707CF8;
        private const ulong H_CLEAR_ENTITY_LAST_DAMAGE_ENTITY = 0xA72CD9CA74A5ECBA;
        private const ulong H_IS_VEHICLE_SEAT_FREE = 0x22AC59A870E6A669;
        private const ulong H_GET_PED_IN_VEHICLE_SEAT = 0xBB40DD2270B65366;
        private const ulong H_GET_VEHICLE_MAX_NUMBER_OF_PASSENGERS = 0xA7C4F2C6E744A550;
        private const ulong H_IS_PED_IN_ANY_VEHICLE = 0x997ABD671D25CA0B;

        private const ulong H_TASK_FOLLOW_TO_OFFSET_OF_ENTITY = 0x304AE42E357B8C7E;
        private const ulong H_TASK_COMBAT_PED = 0xF166E48407BAC484;
        private const ulong H_TASK_ENTER_VEHICLE = 0xC20E50AA46D09CA8;
        private const ulong H_TASK_LEAVE_VEHICLE = 0xD3DBCE61A490BE02;
        private const ulong H_TASK_VEHICLE_ESCORT = 0x0FA6E4B75F302400;
        private const ulong H_TASK_VEHICLE_FOLLOW = 0xFC545A9F0626E3B6;
        private const ulong H_TASK_VEHICLE_DRIVE_TO_COORD_LONGRANGE = 0x158BB33F920D360C;
        private const ulong H_TASK_VEHICLE_MISSION_PED_TARGET = 0x9454528DF15D657A;
        private const ulong H_TASK_HELI_MISSION = 0xDAD029E187A2BEB4;
        private const ulong H_TASK_STAND_GUARD = 0xAE032F8BBA959E90;
        private const ulong H_TASK_GO_STRAIGHT_TO_COORD = 0xD76B57B44F1E6F8B;
        private const ulong H_TASK_SMART_FLEE_PED = 0x22B0D0E37CCB840D;
        private const ulong H_TASK_WANDER_STANDARD = 0xBB9CE077274F6A1B;
        private const ulong H_TASK_TURN_PED_TO_FACE_ENTITY = 0x5AD23D40115353AC;
        private const ulong H_CLEAR_PED_TASKS = 0xE1EF3C1216AFF2CD;
        private const ulong H_CLEAR_PED_TASKS_IMMEDIATELY = 0xAAA34F8A7CB32098;

        private const ulong H_ADD_RELATIONSHIP_GROUP = 0xF372BC22FCB88606;
        private const ulong H_SET_RELATIONSHIP_BETWEEN_GROUPS = 0xBF25EB89375A37AD;

        private const ulong H_SET_VEHICLE_ENGINE_ON = 0x2497C4717C8B881E;
        private const ulong H_SET_VEHICLE_DOORS_LOCKED = 0xB664292EAECF7FA6;
        private const ulong H_SET_VEHICLE_ON_GROUND_PROPERLY = 0x49733E92263139D1;
        private const ulong H_SET_VEHICLE_TYRES_CAN_BURST = 0xEB9DC3C7D8596C46;
        private const ulong H_SET_VEHICLE_MOD_KIT = 0x1F2AA07F00B3217A;
        private const ulong H_SET_VEHICLE_WINDOW_TINT = 0x57C51E6BAD752696;
        private const ulong H_SET_VEHICLE_COLOURS = 0x4F1D4BE3A7F24601;
        private const ulong H_SET_HELI_BLADES_FULL_SPEED = 0xA178472EBB8AE60D;
        private const ulong H_SET_ENTITY_INVINCIBLE = 0x3882114BDE571AD4;
        private const ulong H_SET_ENTITY_AS_MISSION_ENTITY = 0xAD738C3085FE7E11;
        private const ulong H_GET_OFFSET_FROM_ENTITY_IN_WORLD_COORDS = 0x1899F328B0E12848;
        private const ulong H_GET_NTH_CLOSEST_VEHICLE_NODE = 0xE50E52416CCF948B;
        private const ulong H_GET_SAFE_COORD_FOR_PED = 0xB61C8E878A4199CA;
        private const ulong H_GET_GROUND_Z_FOR_3D_COORD = 0xC906A7DAB05C8D2B;

        private const ulong H_SET_BLIP_SPRITE = 0xDF735600A4696DAF;
        private const ulong H_SET_BLIP_COLOUR = 0x03D7FB09E75D6B7E;
        private const ulong H_SET_BLIP_SCALE = 0xD38744167B2FA257;
        private const ulong H_SET_BLIP_AS_SHORT_RANGE = 0xBE8BE4FE60E27B72;
        private const ulong H_BEGIN_TEXT_COMMAND_SET_BLIP_NAME = 0xF9113A30DE5C6670;
        private const ulong H_ADD_TEXT_COMPONENT_SUBSTRING_PLAYER_NAME = 0x6C188BE134E074AA;
        private const ulong H_END_TEXT_COMMAND_SET_BLIP_NAME = 0xBC38B49BCB83BC9B;

        // ------------------------------------------------------------------
        // Ped ayarlari
        // ------------------------------------------------------------------
        public static void SetArmour(int ped, int value) { Call(H_SET_PED_ARMOUR, ped, value); }
        public static void SetMaxHealth(int ped, int value) { Call(H_SET_PED_MAX_HEALTH, ped, value); }
        public static void SetHealth(int entity, int value) { Call(H_SET_ENTITY_HEALTH, entity, value); }
        public static void SetAccuracy(int ped, int value) { Call(H_SET_PED_ACCURACY, ped, value); }
        public static void SetCombatAttribute(int ped, int attribute, bool enabled) { Call(H_SET_PED_COMBAT_ATTRIBUTES, ped, attribute, enabled); }
        public static void SetCombatAbility(int ped, int level) { Call(H_SET_PED_COMBAT_ABILITY, ped, level); }
        public static void SetCombatMovement(int ped, int level) { Call(H_SET_PED_COMBAT_MOVEMENT, ped, level); }
        public static void SetCombatRange(int ped, int range) { Call(H_SET_PED_COMBAT_RANGE, ped, range); }
        public static void SetSeeingRange(int ped, float value) { Call(H_SET_PED_SEEING_RANGE, ped, value); }
        public static void SetHearingRange(int ped, float value) { Call(H_SET_PED_HEARING_RANGE, ped, value); }
        public static void SetAlertness(int ped, int value) { Call(H_SET_PED_ALERTNESS, ped, value); }
        public static void SetShootRate(int ped, int value) { Call(H_SET_PED_SHOOT_RATE, ped, value); }
        public static void SetFiringPattern(int ped, uint pattern) { Call(H_SET_PED_FIRING_PATTERN, ped, unchecked((int)pattern)); }
        public static void SetCanSwitchWeapon(int ped, bool toggle) { Call(H_SET_PED_CAN_SWITCH_WEAPON, ped, toggle); }
        public static void SetKeepTask(int ped, bool toggle) { Call(H_SET_PED_KEEP_TASK, ped, toggle); }
        public static void SetSuffersCriticalHits(int ped, bool toggle) { Call(H_SET_PED_SUFFERS_CRITICAL_HITS, ped, toggle); }
        public static void SetDiesWhenInjured(int ped, bool toggle) { Call(H_SET_PED_DIES_WHEN_INJURED, ped, toggle); }
        public static void SetCanRagdoll(int ped, bool toggle) { Call(H_SET_PED_CAN_RAGDOLL, ped, toggle); }
        public static void SetCanBeTargetted(int ped, bool toggle) { Call(H_SET_PED_CAN_BE_TARGETTED, ped, toggle); }
        public static void SetConfigFlag(int ped, int flag, bool toggle) { Call(H_SET_PED_CONFIG_FLAG, ped, flag, toggle); }
        public static void SetRelationshipGroup(int ped, int groupHash) { Call(H_SET_PED_RELATIONSHIP_GROUP_HASH, ped, groupHash); }
        public static int GetRelationshipGroup(int ped) { return Call<int>(H_GET_PED_RELATIONSHIP_GROUP_HASH, ped); }
        public static void SetCanAttackFriendly(int ped, bool toggle, bool p2) { Call(H_SET_CAN_ATTACK_FRIENDLY, ped, toggle, p2); }
        public static void SetInfiniteAmmo(int ped, bool toggle, int weaponHash) { Call(H_SET_PED_INFINITE_AMMO, ped, toggle, weaponHash); }
        public static void SetInfiniteAmmoClip(int ped, bool toggle) { Call(H_SET_PED_INFINITE_AMMO_CLIP, ped, toggle); }
        public static void SetCurrentWeapon(int ped, int weaponHash, bool equipNow) { Call(H_SET_CURRENT_PED_WEAPON, ped, weaponHash, equipNow); }
        public static void SetCanBeKnockedOffVehicle(int ped, int state) { Call(H_SET_PED_CAN_BE_KNOCKED_OFF_VEHICLE, ped, state); }
        public static void SetRandomOutfit(int ped) { Call(H_SET_PED_RANDOM_COMPONENT_VARIATION, ped, 0); }
        public static void SetComponent(int ped, int component, int drawable, int texture) { Call(H_SET_PED_COMPONENT_VARIATION, ped, component, drawable, texture, 0); }
        public static void SetHelmet(int ped, bool toggle) { Call(H_SET_PED_HELMET, ped, toggle); }
        public static void SetIntoVehicle(int ped, int vehicle, int seat) { Call(H_SET_PED_INTO_VEHICLE, ped, vehicle, seat); }
        public static void SetDriverAbility(int ped, float value) { Call(H_SET_DRIVER_ABILITY, ped, value); }
        public static void SetDriverAggressiveness(int ped, float value) { Call(H_SET_DRIVER_AGGRESSIVENESS, ped, value); }

        // ------------------------------------------------------------------
        // Sorgular
        // ------------------------------------------------------------------
        public static bool IsPedInCombatWith(int ped, int target) { return Call<bool>(H_IS_PED_IN_COMBAT, ped, target); }
        public static bool IsPedShooting(int ped) { return Call<bool>(H_IS_PED_SHOOTING, ped); }
        public static int GetPedType(int ped) { return Call<int>(H_GET_PED_TYPE, ped); }
        public static bool HasBeenDamagedBy(int entity, int by) { return Call<bool>(H_HAS_ENTITY_BEEN_DAMAGED_BY_ENTITY, entity, by, 1); }
        public static void ClearLastDamageEntity(int entity) { Call(H_CLEAR_ENTITY_LAST_DAMAGE_ENTITY, entity); }
        public static bool IsSeatFree(int vehicle, int seat) { return Call<bool>(H_IS_VEHICLE_SEAT_FREE, vehicle, seat); }
        public static int GetPedInSeat(int vehicle, int seat) { return Call<int>(H_GET_PED_IN_VEHICLE_SEAT, vehicle, seat); }
        public static int GetMaxPassengers(int vehicle) { return Call<int>(H_GET_VEHICLE_MAX_NUMBER_OF_PASSENGERS, vehicle); }
        public static bool IsPedInAnyVehicle(int ped) { return Call<bool>(H_IS_PED_IN_ANY_VEHICLE, ped, false); }

        // ------------------------------------------------------------------
        // Gorevler (tasks)
        // ------------------------------------------------------------------
        public static void TaskFollowToOffsetOfEntity(int ped, int entity, Vector3 offset, float speed, int timeout, float stoppingRange, bool persist)
        {
            Call(H_TASK_FOLLOW_TO_OFFSET_OF_ENTITY, ped, entity, offset.X, offset.Y, offset.Z, speed, timeout, stoppingRange, persist);
        }

        public static void TaskCombatPed(int ped, int target)
        {
            Call(H_TASK_COMBAT_PED, ped, target, 0, 16);
        }

        public static void TaskEnterVehicle(int ped, int vehicle, int timeout, int seat, float speed, int flag)
        {
            Call(H_TASK_ENTER_VEHICLE, ped, vehicle, timeout, seat, speed, flag, 0);
        }

        public static void TaskLeaveVehicle(int ped, int vehicle, int flags)
        {
            Call(H_TASK_LEAVE_VEHICLE, ped, vehicle, flags);
        }

        /// <summary>
        /// mode (resmi natives dokumantasyonuyla dogrulandi):
        /// -1 = arkada, 0 = onde, 1 = tam sol, 2 = tam sag, 3 = sol-arka capraz, 4 = sag-arka capraz.
        /// Normal iki seritli yollarda tam sol/sag (1/2) araci karsi seride iter; konvoy
        /// dizilimlerinde capraz pozisyonlar (3/4) tercih edilir.
        /// </summary>
        public static void TaskVehicleEscort(int driver, int vehicle, int targetVehicle, int mode, float speed, int drivingStyle, float minDistance, float noRoadsDistance)
        {
            Call(H_TASK_VEHICLE_ESCORT, driver, vehicle, targetVehicle, mode, speed, drivingStyle, minDistance, 0, noRoadsDistance);
        }

        public static void TaskVehicleFollow(int driver, int vehicle, int targetEntity, float speed, int drivingStyle, int minDistance)
        {
            Call(H_TASK_VEHICLE_FOLLOW, driver, vehicle, targetEntity, speed, drivingStyle, minDistance);
        }

        public static void TaskVehicleDriveToCoord(int driver, int vehicle, Vector3 pos, float speed, int drivingStyle, float stopRange)
        {
            Call(H_TASK_VEHICLE_DRIVE_TO_COORD_LONGRANGE, driver, vehicle, pos.X, pos.Y, pos.Z, speed, drivingStyle, stopRange);
        }

        public static void TaskVehicleMissionPedTarget(int driver, int vehicle, int targetPed, int mission, float speed, int drivingStyle, float minDistance)
        {
            Call(H_TASK_VEHICLE_MISSION_PED_TARGET, driver, vehicle, targetPed, mission, speed, drivingStyle, minDistance, 5f, true);
        }

        /// <summary>
        /// TASK_HELI_MISSION. missionType, TASK_VEHICLE_MISSION ile ayni numaralandirmayi
        /// paylasir (dogrulanmis degerler icin <see cref="HeliMission"/> sabitlerini kullanin).
        /// missionFlags icin <see cref="HeliMissionFlags"/> bit bayraklarini kullanin
        /// (ozellikle inis gorevlerinde LandOnArrival | DontDoAvoidance olmadan helikopter
        /// yere degmeden havada asili kalir).
        /// </summary>
        public static void TaskHeliMission(int pilot, int heli, int targetVehicle, int targetPed, Vector3 pos, int missionType,
                                           float maxSpeed, float radius, float heading, int maxHeight, int minHeight,
                                           int missionFlags = 0, float slowDistance = -1f)
        {
            Call(H_TASK_HELI_MISSION, pilot, heli, targetVehicle, targetPed, pos.X, pos.Y, pos.Z,
                 missionType, maxSpeed, radius, heading, maxHeight, minHeight, slowDistance, missionFlags);
        }

        public static void TaskStandGuard(int ped, Vector3 pos, float heading, string scenario)
        {
            Call(H_TASK_STAND_GUARD, ped, pos.X, pos.Y, pos.Z, heading, scenario);
        }

        public static void TaskGoStraightToCoord(int ped, Vector3 pos, float speed, int timeout, float heading)
        {
            Call(H_TASK_GO_STRAIGHT_TO_COORD, ped, pos.X, pos.Y, pos.Z, speed, timeout, heading, 0f);
        }

        public static void TaskSmartFlee(int ped, int target, float distance, int time)
        {
            Call(H_TASK_SMART_FLEE_PED, ped, target, distance, time, false, false);
        }

        public static void TaskWander(int ped)
        {
            Call(H_TASK_WANDER_STANDARD, ped, 10f, 10);
        }

        public static void TaskTurnToFaceEntity(int ped, int entity, int duration)
        {
            Call(H_TASK_TURN_PED_TO_FACE_ENTITY, ped, entity, duration);
        }

        public static void ClearTasks(int ped) { Call(H_CLEAR_PED_TASKS, ped); }
        public static void ClearTasksImmediately(int ped) { Call(H_CLEAR_PED_TASKS_IMMEDIATELY, ped); }

        // ------------------------------------------------------------------
        // Iliski gruplari
        // ------------------------------------------------------------------
        public static void AddRelationshipGroup(string name)
        {
            OutputArgument outArg = new OutputArgument();
            Call(H_ADD_RELATIONSHIP_GROUP, name, outArg);
        }

        public static void SetRelationshipBetweenGroups(int relation, int group1, int group2)
        {
            Call(H_SET_RELATIONSHIP_BETWEEN_GROUPS, relation, group1, group2);
        }

        // ------------------------------------------------------------------
        // Arac ayarlari
        // ------------------------------------------------------------------
        public static void SetEngineOn(int vehicle, bool on) { Call(H_SET_VEHICLE_ENGINE_ON, vehicle, on, true, false); }
        public static void SetDoorsLocked(int vehicle, int state) { Call(H_SET_VEHICLE_DOORS_LOCKED, vehicle, state); }
        public static void PlaceOnGroundProperly(int vehicle) { Call(H_SET_VEHICLE_ON_GROUND_PROPERLY, vehicle); }
        public static void SetTyresCanBurst(int vehicle, bool toggle) { Call(H_SET_VEHICLE_TYRES_CAN_BURST, vehicle, toggle); }
        public static void SetModKit(int vehicle, int kit) { Call(H_SET_VEHICLE_MOD_KIT, vehicle, kit); }
        public static void SetWindowTint(int vehicle, int tint) { Call(H_SET_VEHICLE_WINDOW_TINT, vehicle, tint); }
        public static void SetColours(int vehicle, int primary, int secondary) { Call(H_SET_VEHICLE_COLOURS, vehicle, primary, secondary); }
        public static void SetHeliBladesFullSpeed(int vehicle) { Call(H_SET_HELI_BLADES_FULL_SPEED, vehicle); }
        public static void SetInvincible(int entity, bool toggle) { Call(H_SET_ENTITY_INVINCIBLE, entity, toggle); }
        public static void SetAsMissionEntity(int entity) { Call(H_SET_ENTITY_AS_MISSION_ENTITY, entity, true, true); }

        // ------------------------------------------------------------------
        // Konum yardimcilari
        // ------------------------------------------------------------------
        public static Vector3 GetOffsetInWorldCoords(int entity, Vector3 offset)
        {
            return Call<Vector3>(H_GET_OFFSET_FROM_ENTITY_IN_WORLD_COORDS, entity, offset.X, offset.Y, offset.Z);
        }

        public static bool GetNthClosestVehicleNode(Vector3 pos, int nth, out Vector3 result)
        {
            OutputArgument outArg = new OutputArgument();
            bool found = Call<bool>(H_GET_NTH_CLOSEST_VEHICLE_NODE, pos.X, pos.Y, pos.Z, nth, outArg, 1, 0x40400000, 0);
            result = found ? outArg.GetResult<Vector3>() : pos;
            return found;
        }

        public static bool GetSafeCoordForPed(Vector3 pos, bool onGround, out Vector3 result)
        {
            OutputArgument outArg = new OutputArgument();
            bool found = Call<bool>(H_GET_SAFE_COORD_FOR_PED, pos.X, pos.Y, pos.Z, onGround, outArg, 16);
            result = found ? outArg.GetResult<Vector3>() : pos;
            return found;
        }

        public static float GetGroundZ(Vector3 pos, float fallback)
        {
            OutputArgument outArg = new OutputArgument();
            bool ok = Call<bool>(H_GET_GROUND_Z_FOR_3D_COORD, pos.X, pos.Y, pos.Z + 2f, outArg, false, false);
            return ok ? outArg.GetResult<float>() : fallback;
        }

        // ------------------------------------------------------------------
        // Blip yardimcilari
        // ------------------------------------------------------------------
        public static void SetBlipSprite(int blip, int sprite) { Call(H_SET_BLIP_SPRITE, blip, sprite); }
        public static void SetBlipColour(int blip, int colour) { Call(H_SET_BLIP_COLOUR, blip, colour); }
        public static void SetBlipScale(int blip, float scale) { Call(H_SET_BLIP_SCALE, blip, scale); }
        public static void SetBlipShortRange(int blip, bool toggle) { Call(H_SET_BLIP_AS_SHORT_RANGE, blip, toggle); }

        public static void SetBlipName(int blip, string name)
        {
            Call(H_BEGIN_TEXT_COMMAND_SET_BLIP_NAME, "STRING");
            Call(H_ADD_TEXT_COMPONENT_SUBSTRING_PLAYER_NAME, name);
            Call(H_END_TEXT_COMMAND_SET_BLIP_NAME, blip);
        }
    }
}
