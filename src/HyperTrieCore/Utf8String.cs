using System.Text;

namespace HyperTrieCore;

/// <summary>
/// Helper to marshal C# string to UTF8 IntPtr with disposal.
/// </summary>
internal unsafe ref struct Utf8String : IDisposable
{
    private fixed byte _fixedBuffer[256];
    public byte* Pointer { get; private set; }
    public int Length { get; private set; }

    public Utf8String(string str)
    {
        if (string.IsNullOrEmpty(str))
        {
            Pointer = null;
            Length = 0;
            return;
        }

        int byteCount = Encoding.UTF8.GetByteCount(str);

        if (byteCount >= 256)
        {
            throw new ArgumentException($"{nameof(Utf8String)} is too long.");
        }

        fixed (byte* pBuffer = _fixedBuffer)
        {
            fixed (char* pStr = str)
            {
                Encoding.UTF8.GetBytes(pStr, str.Length, pBuffer, byteCount);
                pBuffer[byteCount] = 0;
                Pointer = pBuffer;
                Length = byteCount;
            }
        }
    }

    public void Dispose() => Pointer = null;
}
