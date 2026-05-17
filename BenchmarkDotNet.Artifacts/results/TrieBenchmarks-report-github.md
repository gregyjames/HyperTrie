```

BenchmarkDotNet v0.15.8, Linux Ubuntu 24.04.4 LTS (Noble Numbat)
Intel Xeon Processor 2.30GHz, 1 CPU, 4 logical and 4 physical cores
.NET SDK 10.0.103
  [Host] : .NET 10.0.3 (10.0.3, 10.0.326.7603), X64 RyuJIT x86-64-v3

Toolchain=InProcessEmitToolchain

```
| Method               | Mean      | Error    | StdDev   | Rank | Gen0      | Gen1      | Gen2      | Allocated   |
|--------------------- |----------:|---------:|---------:|-----:|----------:|----------:|----------:|------------:|
| &#39;TrieNet (C#)&#39;       | 325.76 ms | 3.631 ms | 3.219 ms |    2 | 5000.0000 | 4000.0000 | 1000.0000 | 109637608 B |
| &#39;HyperTrie (Native)&#39; |  60.02 ms | 0.848 ms | 0.793 ms |    1 |         - |         - |         - |       429 B |
