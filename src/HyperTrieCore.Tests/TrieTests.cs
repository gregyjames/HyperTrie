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
        
        var results = trie.GetWordsWithPrefix("app").ToList();
        
        Assert.Equal(3, results.Count);
        Assert.Contains("apple", results);
        Assert.Contains("app", results);
        Assert.Contains("application", results);
    }

    [Fact]
    public void TestLongStringInsertionAndContains()
    {
        using var trie = new TrieNative(100, 3);

        // 1. String > 64 chars
        string longString = new string('a', 85);
        trie.Insert(longString);
        Assert.True(trie.Contains(longString));
        Assert.False(trie.Contains(new string('a', 84)));

        // 2. Long string with invalid characters and upper case to hit all branches
        string longMixed = "ABC123xyz " + new string('m', 70) + " DEF456";
        trie.Insert(longMixed);

        Assert.True(trie.Contains(longMixed));
        Assert.True(trie.Contains(longMixed.ToLower()));
        Assert.True(trie.Contains(longMixed.ToUpper()));

        // 3. Long missing string
        Assert.False(trie.Contains(new string('z', 90)));
    }

    [Fact]
    public void TestLongStringBulkInsertionAndPrefix()
    {
        using var trie = new TrieNative(100, 3);
        string prefix = "longprefix" + new string('x', 60); // 70 chars prefix
        string longString1 = prefix + "abc";
        string longString2 = prefix + "def";

        trie.BulkInsert([longString1, longString2]);

        Assert.True(trie.Contains(longString1));
        Assert.True(trie.Contains(longString2));

        var results = trie.GetWordsWithPrefix(prefix).ToList();
        Assert.Equal(2, results.Count);
        Assert.Contains(longString1, results);
        Assert.Contains(longString2, results);
    }
}
