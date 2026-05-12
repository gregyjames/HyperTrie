```

BenchmarkDotNet v0.15.8, Linux Ubuntu 24.04.4 LTS (Noble Numbat)
Intel Xeon Processor 2.30GHz, 1 CPU, 4 logical and 4 physical cores
.NET SDK 10.0.103
  [Host] : .NET 10.0.3 (10.0.3, 10.0.326.7603), X64 RyuJIT x86-64-v3

Toolchain=InProcessEmitToolchain

```
| Method               | Mean      | Error    | StdDev   | Rank | Gen0      | Gen1      | Gen2      | Allocated   |
|--------------------- |----------:|---------:|---------:|-----:|----------:|----------:|----------:|------------:|
| &#39;TrieNet (C#)&#39;       | 324.79 ms | 4.898 ms | 4.342 ms |    2 | 5000.0000 | 4000.0000 | 1000.0000 | 109637608 B |
| &#39;HyperTrie (Native)&#39; |  47.77 ms | 0.306 ms | 0.286 ms |    1 |         - |         - |         - |       325 B |
