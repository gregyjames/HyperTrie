![Alt text](https://raw.githubusercontent.com/gregyjames/HyperTrie/main/mini_trie.png "package icon")
# HyperTrie
HyperTrie is a hyper optimized C# prefix tree written in Rust. It is currently the fastest C# Trie implementation, about 601% faster than TrieNet.Core 😮‍💨

## Why make this?
Well, I wanted to try optimizing some of the hot paths in one of my libraries [Octane Downloader](https://github.com/gregyjames/OctaneDownloader) by rewritting them in Rust, but in order to do that, I needed a simpler project to experiment with multi-target builds and including native rust code in a Nuget package. Then I proceeded to optimize the hell out of it for no reason just to see how far I could go. I'm sure there potentially more optimizations to make, like using u8 instead of char for space complexity, so if you see anything feel free to open a PR.

### Why not use a HashMap?
Because of the additional overhead of hashing and collisions. It's faster to just use an array. Although, this leads to poor space complexity due to sparse arrays everywhere, which is why I decided to only support 26 characters.

### Why a bloom filter?
This is my favorite data structure, and it made perfect sense here since it will never give false negatives. It seemed like the obvious choice to side step hashing and checking if an entry exists in the Trie.

## Installation
```bash
dotnet add package HyperTrieCore --version 1.0.27
```

## Example
```csharp
const string url = "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt";
var client = new HttpClient();
var content = client.GetStringAsync(url).Result;
var allWords = content.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).Select(x => x.ToLower().Trim()).ToList();
var trieNative = new TrieNative(allWords.Count(), 3);
trieNative.BulkInsert(allWords);
```

## Benchmark
```

BenchmarkDotNet v0.14.0, macOS 26.1 (25B78) [Darwin 25.1.0]
Apple M1, 1 CPU, 8 logical and 8 physical cores
.NET SDK 8.0.100
  [Host] : .NET 8.0.0 (8.0.23.53103), Arm64 RyuJIT AdvSIMD

Toolchain=InProcessEmitToolchain  

```
| Method               | Mean      | Error    | StdDev   | Rank | Gen0       | Gen1      | Gen2      | Allocated |
|--------------------- |----------:|---------:|---------:|-----:|-----------:|----------:|----------:|----------:|
| &#39;TrieNet (C#)&#39;       | 226.32 ms | 3.409 ms | 3.022 ms |    2 | 19500.0000 | 8000.0000 | 3000.0000 | 104.56 MB |
| &#39;HyperTrie (Native)&#39; |  38.52 ms | 0.744 ms | 0.764 ms |    1 |   538.4615 |  538.4615 |  153.8462 |   2.06 MB |


## Limitations

 1. No OSX64 support, the Rust code uses GXHash and the Github actions runner does not support the necessary CPU instruction sets :(
 2. To maximize performance it only supports 26 ASCII characters (A-Z), this is optimal for usecases such as a dictionary or spellcheck applications but not really useful for something like checking available usernames.

## Local development
Building is simplified via [Nuke](https://nuke.build/). Use the following commands:

```bash
./build.sh --help         # Show all available targets
./build.sh Compile        # Build the C# projects (Default target)
./build.sh BuildRustAll   # Build Rust native libraries for all platforms (requires `cargo cross`)
./build.sh Pack           # Compiles all C# libraries and generates .nupkg NuGet files
./build.sh PublishNuGet   # Pack and push packages to NuGet.org (Requires API key)
```

**Build Outputs**
This project relies on MSBuild `UseArtifactsOutput` and maps all builds into a centralized artifacts directory. 
After compilation, check the `artifacts/` folder at the repository root. Native Rust binary artifacts will be cleanly routed to `artifacts/native/`, while bundled NuGet files are pushed to `artifacts/packages/`.
## License
MIT License

Copyright (c) 2025 Greg James

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
