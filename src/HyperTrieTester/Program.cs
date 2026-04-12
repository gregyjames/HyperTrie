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
    private const string URL = "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt";
    private const int NUM_TRIES = 500;

    private List<string> _allWords = null!;
    private int[] _randomIndices = null!;
    private static readonly char[] Separator = ['\r', '\n'];

    [GlobalSetup]
    public void GlobalSetup()
    {
        using var client = new HttpClient();
        string content = client.GetStringAsync(URL).Result;
        _allWords = content
            .Split(Separator, StringSplitOptions.RemoveEmptyEntries)
            .Select(x => x.ToLower().Trim())
            .ToList();

        var random = new Random(42);
        _randomIndices = Enumerable.Range(0, NUM_TRIES)
            .Select(_ => random.Next(0, _allWords.Count))
            .ToArray();
    }

    [Benchmark(Description = "TrieNet (C#)")]
    public void TrieNetCSharp()
    {
        var trie = new Trie<string>();

        foreach (string word in _allWords)
        {
            trie.Add(word, word);
        }


        for (int i = 0; i < NUM_TRIES; i++)
        {
            int indx = _randomIndices[i];
            trie.Retrieve(_allWords[indx]);
        }
    }

    [Benchmark(Description = "HyperTrie (Native)")]
    public void TrieNativeBenchmark()
    {
        using var trie = new TrieNative(_allWords.Count, 3);

        trie.BulkInsert(_allWords);

        for (int i = 0; i < NUM_TRIES; i++)
        {
            int indx = _randomIndices[i];
            trie.Contains(_allWords[indx]);
        }
    }
}
