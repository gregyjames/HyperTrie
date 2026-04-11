using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Nuke.Common;
using Nuke.Common.CI;
using Nuke.Common.CI.GitHubActions;
using Nuke.Common.IO;
using Nuke.Common.ProjectModel;
using Nuke.Common.Tooling;
using Nuke.Common.Tools.DotNet;
using Nuke.Common.Tools.GitVersion;
using Nuke.Common.Utilities.Collections;
using Serilog;
using static Nuke.Common.EnvironmentInfo;
using static Nuke.Common.IO.PathConstruction;
using static Nuke.Common.Tools.DotNet.DotNetTasks;

class Build : NukeBuild
{
    public static int Main() => Execute<Build>(x => x.Compile);

    [Parameter("Configuration to build - Default is 'Debug' (local) or 'Release' (server)")]
    readonly Configuration Config = IsLocalBuild ? Configuration.Debug : Configuration.Release;

    [Solution(GenerateProjects = true)]
    readonly Solution Solution!;

    [GitVersion]
    readonly GitVersion GitVersion!;

    [Parameter("Version for NuGet package")]
    readonly string? Version;

    [Parameter("NuGet API key")]
    readonly string? NuGetApiKey;

    [Parameter("Rust target triple to build (single platform CI mode)")]
    readonly string? RustTarget;

    [Parameter("Runtime identifier for the current platform")]
    readonly string? RuntimeId;

    [Parameter("Library filename produced by cargo")]
    readonly string? LibName;

    AbsolutePath HyperTrieCorePath => RootDirectory / "HyperTrieCore";
    AbsolutePath RustProjectPath => RootDirectory / "hypertrie";
    AbsolutePath NativeOutputPath => HyperTrieCorePath / "target" / "release";
    AbsolutePath SourcePath => HyperTrieCorePath / "src";
    AbsolutePath LibraryProjectPath => SourcePath / "HyperTrieCore" / "HyperTrieCore.csproj";
    AbsolutePath PackageOutputPath => SourcePath / "HyperTrieCore" / "bin" / "Release";

    // Platform definitions for local cross-compilation
    static readonly (string Rid, string Target, string LibName, string RustFlags)[] Platforms =
    [
        ("linux-x64",   "x86_64-unknown-linux-gnu",  "libhypertrie.so",     "-C target-feature=+aes,+sse2"),
        ("windows-x64", "x86_64-pc-windows-msvc",    "hypertrie.dll",       "-C target-feature=+aes,+sse2"),
        ("windows-x86", "i686-pc-windows-msvc",      "hypertrie.dll",       "-C target-feature=+aes,+sse2"),
        ("osx-x64",     "x86_64-apple-darwin",        "libhypertrie.dylib",  ""),
        ("osx-arm64",   "aarch64-apple-darwin",       "libhypertrie.dylib",  ""),
    ];

    Target Clean => _ => _
        .Before(Restore)
        .Executes(() =>
        {
            DotNetClean(s => s.SetProject(Solution));

            if (NativeOutputPath.Exists())
                NativeOutputPath.DeleteDirectory();
        });

    Target Restore => _ => _
        .Executes(() =>
        {
            DotNetRestore(s => s.SetProjectFile(Solution));
        });

    /// <summary>
    /// Builds the Rust native library for a single platform.
    /// Used by CI where each runner builds one target via matrix.
    /// Pass --rust-target, --runtime-id, and --lib-name parameters.
    /// </summary>
    Target BuildRustSingle => _ => _
        .Requires(() => RustTarget)
        .Requires(() => RuntimeId)
        .Requires(() => LibName)
        .Executes(() =>
        {
            var platform = Platforms.FirstOrDefault(p => p.Target == RustTarget);
            var rustFlags = platform != default ? platform.RustFlags : "";

            BuildRustForTarget(RustTarget, RuntimeId, LibName, rustFlags);
        });

    /// <summary>
    /// Builds the Rust native library for ALL platforms (local dev cross-compile).
    /// Requires all cross-compilation toolchains installed locally.
    /// </summary>
    Target BuildRustAll => _ => _
        .Executes(() =>
        {
            foreach (var (rid, target, libname, rustFlags) in Platforms)
            {
                Serilog.Log.Information("Building Rust library for {Rid} ({Target})...", rid, target);

                // Ensure the rust target is installed
                ProcessTasks.StartProcess("rustup", $"target add {target}", RustProjectPath)
                    .AssertZeroExitCode();

                BuildRustForTarget(target, rid, libname, rustFlags);
            }
        });

    Target Compile => _ => _
        .DependsOn(Restore)
        .Executes(() =>
        {
            var version = Version ?? GitVersion.NuGetVersionV2;
            Serilog.Log.Information("Building version {Version}...", version);

            DotNetBuild(s => s
                .SetProjectFile(Solution)
                .SetConfiguration(Config)
                .SetNoRestore(true)
                .SetProperty("Version", version));
        });

    Target Pack => _ => _
        .DependsOn(Compile)
        .Executes(() =>
        {
            var version = Version ?? GitVersion.NuGetVersionV2;
            Serilog.Log.Information("Packing version {Version}...", version);

            DotNetPack(s => s
                .SetProject(LibraryProjectPath)
                .SetConfiguration(Configuration.Release)
                .SetVersion(version)
                .SetNoBuild(true));
        });

    Target PublishNuGet => _ => _
        .DependsOn(Pack)
        .Requires(() => NuGetApiKey)
        .Executes(() =>
        {
            var packages = Directory.GetFiles(PackageOutputPath, "*.nupkg");
            foreach (var package in packages)
            {
                DotNetNuGetPush(s => s
                    .SetTargetPath(package)
                    .SetSource("https://api.nuget.org/v3/index.json")
                    .SetApiKey(NuGetApiKey)
                    .SetSkipDuplicate(true));
            }
        });

    Target PublishGitHub => _ => _
        .DependsOn(Pack)
        .Executes(() =>
        {
            var token = Environment.GetEnvironmentVariable("GITHUB_TOKEN");
            Assert.NotNullOrEmpty(token, "GITHUB_TOKEN environment variable must be set");

            var packages = Directory.GetFiles(PackageOutputPath, "*.nupkg");
            foreach (var package in packages)
            {
                DotNetNuGetPush(s => s
                    .SetTargetPath(package)
                    .SetSource("github")
                    .SetApiKey(token)
                    .SetSkipDuplicate(true));
            }
        });

    void BuildRustForTarget(string target, string runtimeId, string libName, string rustFlags)
    {
        var outputDir = NativeOutputPath / runtimeId;
        Directory.CreateDirectory(outputDir);

        var env = new Dictionary<string, string>();
        if (!string.IsNullOrEmpty(rustFlags))
            env["RUSTFLAGS"] = rustFlags;

        var process = ProcessTasks.StartProcess(
            "cargo",
            $"build --release --target {target}",
            workingDirectory: RustProjectPath,
            environmentVariables: env.Any()
                ? env.ToDictionary(x => x.Key, x => x.Value)
                : null);

        process.AssertZeroExitCode();

        var sourceLib = RustProjectPath / "target" / target / "release" / libName;
        var destLib = outputDir / libName;

        File.Copy(sourceLib, destLib, overwrite: true);
        Serilog.Log.Information("Copied {Source} → {Dest}", sourceLib, destLib);
    }
}
