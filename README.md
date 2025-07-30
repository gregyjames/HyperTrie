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
Took 505ms to run Trie.Net
Took 84ms to run Trie Native
```
The benchmark is ran by inserting 172,819 words from the [Official Scrabble Player's Dictionary](https://github.com/dolph/dictionary) into the Tries and proceeding to check 500 random word occurances.

## Limitations

 1. No OSX64 support, the Rust code uses GXHash and the Github actions runner does not support the necessary CPU instruction sets :(
 2. To maximize performance it only supports 26 ASCII characters (A-Z), this is optimal for usecases such as a dictionary or spellcheck applications but not really useful for something like checking available usernames.

## Local development
The building in VS/Rider is still f***** up, so you will have to use the makefile:
```bash
make run
```

## License
MIT License

Copyright (c) 2025 Greg James

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
