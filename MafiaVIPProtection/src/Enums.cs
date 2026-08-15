namespace MafiaVIP
{
    /// <summary>Yakin koruma ekibinin davranis modu.</summary>
    public enum GuardBehavior
    {
        /// <summary>Menzildeki her dusmani proaktif olarak engaje eder.</summary>
        Aggressive,
        /// <summary>Sadece oyuncuya / ekibe saldiran hedefleri engaje eder.</summary>
        Defensive,
        /// <summary>Ates etmez, sadece formasyonda kalir.</summary>
        Passive
    }

    /// <summary>Konvoy surus modu.</summary>
    public enum ConvoySpeedMode
    {
        Normal,
        Aggressive,
        Stealth
    }

    /// <summary>Yaya formasyonu / konvoy dizilimi.</summary>
    public enum Formation
    {
        /// <summary>Elmas: iki on, iki arka, yanlar.</summary>
        Diamond,
        /// <summary>Kutu: VIP ortada, dort kose.</summary>
        Box,
        /// <summary>Kolon: tek sira arkada.</summary>
        Column,
        /// <summary>Kama: V dizilimi.</summary>
        Wedge,
        /// <summary>360 derece cember savunma.</summary>
        Circle
    }

    /// <summary>Oyuncu oldugunde korumalarin davranisi.</summary>
    public enum DeathBehavior
    {
        /// <summary>Cesedin basinda nobet tutar.</summary>
        Guard,
        /// <summary>Dagilir ve kacar.</summary>
        Scatter,
        /// <summary>Aninda silinir.</summary>
        Vanish
    }

    /// <summary>Hava destegi gorev durumu.</summary>
    public enum AirState
    {
        Idle,
        Escorting,
        Engaging,
        Landing,
        Leaving
    }
}
