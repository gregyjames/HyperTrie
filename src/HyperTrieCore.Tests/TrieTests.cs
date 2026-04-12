using Xunit;
using HyperTrieCore;

namespace HyperTrieCore.Tests;

public class TrieTests
{
    [Fact]
    public void TestBasicInsertion()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("apple");
        trie.Insert("banana");

        Assert.True(trie.Contains("apple"));
        Assert.True(trie.Contains("banana"));
        Assert.False(trie.Contains("orange"));
    }
    
    [Fact]
    public void TestBulkInsertion()
    {
        using var trie = new TrieNative(4, 3);
        trie.BulkInsert(["apple", "banana"]);

        Assert.True(trie.Contains("apple"));
        Assert.True(trie.Contains("banana"));
        Assert.False(trie.Contains("orange"));
    }

    [Fact]
    public void TestPrefixSearch()
    {
        using var trie = new TrieNative(100, 3);
        trie.Insert("apple");
        trie.Insert("app");
        trie.Insert("application");
        
        var results = trie.GetWordsWithPrefix("app");
        
        Assert.Equal(3, results.Count);
        Assert.Contains("apple", results);
        Assert.Contains("app", results);
        Assert.Contains("application", results);
    }
}
