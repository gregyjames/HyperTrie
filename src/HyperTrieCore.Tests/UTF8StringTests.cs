using System;
using System.Runtime.InteropServices;
using System.Text;
using Xunit;

namespace HyperTrieCore.Tests
{
    public class Utf8StringTests
    {
        [Theory]
        [InlineData(null)]
        [InlineData("")]
        public void Constructor_ShouldHandleNullOrEmpty(string? input)
        {
            // Act
            using var utf8 = new Utf8String(input!);

            // Assert
            Assert.Equal(IntPtr.Zero, utf8.Pointer);
        }

        [Fact]
        public void Constructor_ShouldHandleSmallString_StackPath()
        {
            // Arrange
            string input = "Hello Stack";

            // Act
            using var utf8 = new Utf8String(input);

            // Assert
            Assert.NotEqual(IntPtr.Zero, utf8.Pointer);
            string? result = Marshal.PtrToStringAnsi(utf8.Pointer); // Read back to verify
            Assert.Equal(input, result);
        }

        [Fact]
        public void Constructor_ShouldHandleLargeString_HeapPath()
        {
            // Arrange
            // Create a string that exceeds the 256 byte threshold
            // 300 characters will definitely exceed STACK_THRESHOLD
            string input = new string('A', 300);

            // Act
            using var utf8 = new Utf8String(input);

            // Assert
            Assert.NotEqual(IntPtr.Zero, utf8.Pointer);

            // Manual verification of the content at the pointer
            byte[] buffer = new byte[input.Length];
            Marshal.Copy(utf8.Pointer, buffer, 0, input.Length);
            string result = Encoding.UTF8.GetString(buffer);

            Assert.Equal(input, result);

            // Verify null termination
            byte nullTerminator = Marshal.ReadByte(utf8.Pointer, input.Length);
            Assert.Equal(0, nullTerminator);
        }

        [Fact]
        public void Dispose_ShouldResetPointer()
        {
            // Arrange
            var utf8 = new Utf8String("Dispose Test");

            // Act
            utf8.Dispose();

            // Assert
            Assert.Equal(IntPtr.Zero, utf8.Pointer);
        }

        [Fact]
        public unsafe void MemoryContents_ShouldBeValidUtf8()
        {
            // Arrange
            string input = "🚀 High Perf"; // Includes a 4-byte emoji

            // Act
            using var utf8 = new Utf8String(input);

            // Assert
            byte* ptr = (byte*)utf8.Pointer;
            int expectedByteCount = Encoding.UTF8.GetByteCount(input);

            // Verify each byte matches Encoding.UTF8
            byte[] expectedBytes = Encoding.UTF8.GetBytes(input);
            for (int i = 0; i < expectedByteCount; i++)
            {
                Assert.Equal(expectedBytes[i], ptr[i]);
            }

            // Verify null terminator at the correct offset
            Assert.Equal(0, ptr[expectedByteCount]);
        }
    }
}
