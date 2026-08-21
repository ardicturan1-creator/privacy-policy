namespace MafiaVIP
{
    /// <summary>
    /// TASK_HELI_MISSION / TASK_VEHICLE_MISSION'in paylastigi gorev tipi numaralari.
    /// Kaynak: citizenfx/natives resmi dokumantasyonu (TaskVehicleMission.md).
    ///
    /// UYARI: Bu numaralar internet uzerinde cok sik yanlis kopyalanir (ozellikle
    /// Circle=8 ve Attack=4 seklinde). Dogrulanmis degerler asagidadir; yanlislari
    /// kullanmak helikopterin "daire cizecegine kacmasina" (Flee) veya "saldiracagina
    /// sadece bir noktaya ucup duracagina" (GoTo) sebep olur.
    /// </summary>
    internal static class HeliMission
    {
        public const int GoTo = 4;
        public const int Attack = 6;
        public const int Follow = 7;
        public const int Flee = 8;
        public const int Circle = 9;
        public const int Land = 19;
        public const int LandAndWait = 20;
        public const int HeliProtect = 23;
    }

    /// <summary>
    /// eHeliMissionFlags bit bayraklari. TASK_HELI_MISSION'in son parametresi.
    /// Inis gorevlerinde LandOnArrival | DontDoAvoidance verilmezse pilot AI'i
    /// hedefe yaklasir ama havada asili kalir, hicbir zaman yere degmez.
    /// </summary>
    internal static class HeliMissionFlags
    {
        public const int None = 0;
        public const int AttainRequestedOrientation = 1;
        public const int DontModifyOrientation = 2;
        public const int DontModifyPitch = 4;
        public const int DontModifyThrottle = 8;
        public const int DontModifyRoll = 16;
        public const int LandOnArrival = 32;
        public const int DontDoAvoidance = 64;
        public const int StartEngineImmediately = 128;
        public const int ForceHeightMapAvoidance = 256;
        public const int MaintainHeightAboveTerrain = 4096;

        /// <summary>Guvenilir inis icin standart kombinasyon.</summary>
        public const int LandingCombo = LandOnArrival | DontDoAvoidance;
    }

    /// <summary>
    /// TASK_VEHICLE_ESCORT icin dogrulanmis konum modlari.
    /// </summary>
    internal static class EscortMode
    {
        public const int Rear = -1;
        public const int Front = 0;
        public const int Left = 1;
        public const int Right = 2;
        public const int RearLeft = 3;
        public const int RearRight = 4;
    }

    /// <summary>
    /// SHVDN'in DrivingStyle enum'undan siklikla kullanilan sabit degerler.
    /// Rushed, "escort minDistance parametresini gormezden gelmez" olarak
    /// belgelenen tek agresif stil oldugu icin Aggressive modda tercih edilir.
    /// </summary>
    internal static class DrivingStylePreset
    {
        public const int Normal = 786603;
        public const int Rushed = 2883621;
        public const int AvoidTrafficExtremely = 6;
    }
}
