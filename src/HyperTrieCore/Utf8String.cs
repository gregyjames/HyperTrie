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
            int maxByteCount = Encoding.UTF8.GetMaxByteCount(str.Length);

            if (maxByteCount < STACK_THRESHOLD)
            {
                fixed (byte* pBuffer = _fixedBuffer)
                {
                    int bytesWritten = Encoding.UTF8.GetBytes(pStr, str.Length, pBuffer, STACK_THRESHOLD);
                    pBuffer[bytesWritten] = 0;
                    Pointer = (nint)pBuffer;
                    Length = bytesWritten;
                }
            }
            else
            {
                // We don't want to over-allocate significantly with GetMaxByteCount if it's large,
                // but for smallish strings it's fine. For larger ones, maybe we should stick to two-pass or just allocate.
                // Actually, for consistency and avoiding double pass:
                int actualByteCount = Encoding.UTF8.GetByteCount(str);
                _allocatedPtr = (byte*)Marshal.AllocHGlobal(actualByteCount + 1);
                _isHeapAllocated = true;
                Encoding.UTF8.GetBytes(pStr, str.Length, _allocatedPtr, actualByteCount);
                _allocatedPtr[actualByteCount] = 0;
                Pointer = (nint)_allocatedPtr;
                Length = actualByteCount;
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
        Length = 0;
    }
}
