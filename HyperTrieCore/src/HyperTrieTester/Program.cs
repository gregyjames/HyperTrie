using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Gma.DataStructures.StringSearch;

namespace HyperTrieCore;
internal static class Program
{
    public static void Main(string[] args)
    {
        const string url = "https://raw.githubusercontent.com/dolph/dictionary/master/enable1.txt";
        const int numTries = 500;
        
        var client = new HttpClient();
        var content = client.GetStringAsync(url).Result;
        var allWords = content.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).Select(x => x.ToLower().Trim()).ToList();
        var trieNative = new TrieNative(allWords.Count(), 3);
        var trieCSharp = new Trie<string>();
        
        Random random = new Random();
        var sw = Stopwatch.StartNew();
        foreach (var word in allWords)
        {
            trieCSharp.Add(word, word);
        }
        
        for (int i = 0; i < numTries; i++)
        {
            var indx = random.Next(0, allWords.Count());
            trieCSharp.Retrieve(indx % 2 == 0 ? allWords[indx] : string.Join("", allWords[indx].Reverse()));
        }
        sw.Stop();
        
        Console.WriteLine($"Took {sw.ElapsedMilliseconds}ms to run Trie.Net");
        
        sw.Restart();
        trieNative.BulkInsert(allWords);
        for (int i = 0; i < numTries; i++)
        {
            var indx = random.Next(0, allWords.Count());
            trieNative.Contains(indx % 2 == 0 ? allWords[indx] : string.Join("", allWords[indx].Reverse()));
        }
        sw.Stop();
        
        Console.WriteLine($"Took {sw.ElapsedMilliseconds}ms to run Trie Native");
    }
}