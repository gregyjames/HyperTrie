using System.Runtime.InteropServices;
using System.Text;

namespace HyperTrieCore;

/// <summary>
/// Helper to marshal C# string to UTF8 IntPtr with disposal.
/// </summary>
internal class Utf8String : IDisposable
{
    public IntPtr Pointer { get; private set; }

    public Utf8String(string str)
    {
        if (str == null) throw new ArgumentNullException(nameof(str));

        // Get byte array directly, with null terminator
        byte[] utf8Bytes = Encoding.UTF8.GetBytes(str);
        int lengthWithNullTerminator = utf8Bytes.Length + 1;  // +1 for the null terminator

        // Allocate memory and copy bytes to unmanaged memory
        Pointer = Marshal.AllocHGlobal(lengthWithNullTerminator);
        Marshal.Copy(utf8Bytes, 0, Pointer, utf8Bytes.Length);
        Marshal.WriteByte(Pointer + utf8Bytes.Length, 0); 
    }
        
    private bool _disposed;

    public void Dispose()
    {
        if (_disposed) return;

        if (Pointer != IntPtr.Zero)
        {
            Marshal.FreeHGlobal(Pointer);
            Pointer = IntPtr.Zero;
        }

        _disposed = true;
        GC.SuppressFinalize(this);
    }
        
    ~Utf8String()
    {
        Dispose();
    }
}