using System.Runtime.InteropServices;
using System.Text;

namespace HyperTrieCore;

/// <summary>
/// Helper to marshal C# string to UTF8 IntPtr with disposal.
/// </summary>
internal unsafe ref struct Utf8String : IDisposable
{
    private const int STACK_THRESHOLD = 256;
    // ReSharper disable once PrivateFieldCanBeConvertedToLocalVariable
    private fixed byte _fixedBuffer[STACK_THRESHOLD];
    private byte* _allocatedPtr;
    private bool _isHeapAllocated;
    public IntPtr Pointer {get; private set;}
    public int Length {get; private set;}

    public Utf8String(string str)
    {
        _allocatedPtr = null;
        _isHeapAllocated = false;
        Length = 0;

        if (string.IsNullOrEmpty(str))
        {
            Pointer = IntPtr.Zero;
            return;
        }

        fixed (char* pStr = str)
        {
            int byteCount = Encoding.UTF8.GetByteCount(str);
            Length = byteCount;
            int requiredSize = byteCount + 1; // Keep +1 for compatibility with tests and safety

            if (requiredSize <= STACK_THRESHOLD)
            {
                fixed (byte* pBuffer = _fixedBuffer)
                {
                    Encoding.UTF8.GetBytes(pStr, str.Length, pBuffer, byteCount);
                    pBuffer[byteCount] = 0;
                    Pointer = (nint)pBuffer;
                }
            }
            else
            {
                _allocatedPtr = (byte*)Marshal.AllocHGlobal(requiredSize);
                _isHeapAllocated = true;
                Encoding.UTF8.GetBytes(pStr, str.Length, _allocatedPtr, byteCount);
                _allocatedPtr[byteCount] = 0;
                Pointer = (nint)_allocatedPtr;
            }
        }
    }

    public void Dispose()
    {
        if (_isHeapAllocated && _allocatedPtr != null)
        {
            Marshal.FreeHGlobal((IntPtr)_allocatedPtr);
            _allocatedPtr = null;
            _isHeapAllocated = false;
        }

        Pointer = IntPtr.Zero;
    }
}
