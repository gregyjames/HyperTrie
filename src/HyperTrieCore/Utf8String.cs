using System.Runtime.InteropServices;
using System.Text;

namespace HyperTrieCore;

/// <summary>
/// Helper to marshal C# string to UTF8 IntPtr with disposal.
/// </summary>
internal unsafe ref struct Utf8String : IDisposable
{
    public IntPtr Pointer { get; private set; }

    public Utf8String(string str)
    {
        if (string.IsNullOrEmpty(str))
        {
            Pointer = IntPtr.Zero;
            return;
        };
        
        fixed (char* pStr = str)
        {
            int byteCount = Encoding.UTF8.GetByteCount(pStr, str.Length);
            
            Pointer = Marshal.AllocHGlobal(byteCount + 1);
            
            byte* pDest = (byte*)Pointer;
            Encoding.UTF8.GetBytes(pStr, str.Length, pDest, byteCount);

            pDest[byteCount] = 0;
        }
    }

    private bool _disposed = false;

    public void Dispose()
    {
        if (_disposed) return;

        if (Pointer != IntPtr.Zero)
        {
            Marshal.FreeHGlobal(Pointer);
            Pointer = IntPtr.Zero;
        }

        _disposed = true;
    }
}