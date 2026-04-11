using BenchmarkDotNet.Attributes;
using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Jobs;
using BenchmarkDotNet.Running;
using BenchmarkDotNet.Toolchains.InProcess.Emit;
using Gma.DataStructures.StringSearch;
using HyperTrieCore;

var config = DefaultConfig.Instance
    .WithOptions(ConfigOptions.DisableOptimizationsValidator)
    .AddJob(Job.Default.WithToolchain(InProcessEmitToolchain.Instance));

BenchmarkRunner.Run<TrieBenchmarks>(config);

[MemoryDiagnoser]
[RankColumn]
public class TrieBenchmarks
{
    private const string Url = "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt";
    private const int NumTries = 500;

    private List<string> _allWords = null!;
    private int[] _randomIndices = null!;

    [GlobalSetup]
    public void GlobalSetup()
    {
        using var client = new HttpClient();
        var content = client.GetStringAsync(Url).Result;
        _allWords = content
            .Split(new char[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
            .Select(x => x.ToLower().Trim())
            .ToList();

        var random = new Random(42);
        _randomIndices = Enumerable.Range(0, NumTries)
            .Select(_ => random.Next(0, _allWords.Count))
            .ToArray();
    }

    [Benchmark(Description = "TrieNet (C#)")]
    public void TrieNetCSharp()
    {
        var trie = new Trie<string>();

        foreach (var word in _allWords)
        {
            trie.Add(word, word);
        }

        for (int i = 0; i < NumTries; i++)
        {
            var indx = _randomIndices[i];
            trie.Retrieve(indx % 2 == 0 ? _allWords[indx] : string.Join("", _allWords[indx].Reverse()));
        }
    }

    [Benchmark(Description = "HyperTrie (Native)")]
    public void TrieNativeBenchmark()
    {
        using var trie = new TrieNative(_allWords.Count, 3);

        trie.BulkInsert(_allWords);

        for (int i = 0; i < NumTries; i++)
        {
            var indx = _randomIndices[i];
            trie.Contains(indx % 2 == 0 ? _allWords[indx] : string.Join("", _allWords[indx].Reverse()));
        }
    }
}