using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using Nuke.Common;
using Nuke.Common.CI;
using Nuke.Common.CI.GitHubActions;
using Nuke.Common.IO;
using Nuke.Common.ProjectModel;
using Nuke.Common.Tooling;
using Nuke.Common.Tools.DotNet;
using Nuke.Common.Utilities.Collections;
using Serilog;
using static Nuke.Common.EnvironmentInfo;
using static Nuke.Common.IO.PathConstruction;
using static Nuke.Common.Tools.DotNet.DotNetTasks;

class Build : NukeBuild
{
    public static int Main() => Execute<Build>(x => x.Compile);

    Target RustTest => _ => _
        .Executes(() =>
        {
            ProcessTasks.StartProcess("cargo", "test", RustProjectPath)
                .AssertZeroExitCode();
        });

    Target Test => _ => _
        .DependsOn(Compile)
        .DependsOn(RustTest)
        .Executes(() =>
        {
            DotNetTest(s => s
                .SetProjectFile(Solution)
                .SetConfiguration(Config)
                .SetNoBuild(true)
                .SetVerbosity(DotNetVerbosity.normal)
                .SetProperty("CollectCoverage", "true")
                .SetProperty("CoverletOutputFormat", "cobertura")
                .SetProperty("CoverletOutput", "coverage"));
        });

    [Parameter("Configuration to build - Default is 'Debug' (local) or 'Release' (server)")]
    readonly Configuration Config = IsLocalBuild ? Configuration.Debug : Configuration.Release;

    [Solution(GenerateProjects = true)]
    readonly Solution Solution = default!;

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

    AbsolutePath SourcePath => RootDirectory / "src";
    AbsolutePath HyperTrieCorePath => SourcePath / "HyperTrieCore";
    AbsolutePath RustProjectPath => SourcePath / "hypertrie";
    AbsolutePath ArtifactsDirectory => RootDirectory / "artifacts";
    AbsolutePath NativeOutputPath => HyperTrieCorePath / "runtimes";
    AbsolutePath LibraryProjectPath => HyperTrieCorePath / "HyperTrieCore.csproj";
    AbsolutePath PackageOutputPath => ArtifactsDirectory / "packages";

    // Platform definitions for local cross-compilation
    static readonly (string Rid, string Target, string LibName, string RustFlags)[] Platforms =
    [
        ("linux-x64",   "x86_64-unknown-linux-gnu",  "libhypertrie.so",     "-C target-feature=+aes,+sse2 -C linker=gcc"),
        ("windows-x64", "x86_64-pc-windows-msvc",    "hypertrie.dll",       "-C target-feature=+aes,+sse2"),
        ("windows-x86", "i686-pc-windows-msvc",      "hypertrie.dll",       "-C target-feature=+aes,+sse2"),
        ("osx-arm64",   "aarch64-apple-darwin",       "libhypertrie.dylib",  ""),
    ];

    Target Clean => _ => _
        .Before(Restore)
        .Executes(() =>
        {
            DotNetClean(s => s.SetProject(Solution));
            ArtifactsDirectory.CreateOrCleanDirectory();
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
            var version = Version ?? "1.0.0";

            // Local development helper: Auto-build native lib if missing
            if (IsLocalBuild)
            {
                var currentRid = RuntimeInformation.IsOSPlatform(OSPlatform.Windows) ? (Environment.Is64BitProcess ? "win-x64" : "win-x86") :
                                 RuntimeInformation.IsOSPlatform(OSPlatform.Linux) ? "linux-x64" :
                                 RuntimeInformation.IsOSPlatform(OSPlatform.OSX) ? "osx-arm64" : null;

                if (currentRid != null)
                {
                    var platform = Platforms.FirstOrDefault(p => p.Rid == currentRid);
                    if (platform != default)
                    {
                        var libPath = NativeOutputPath / platform.Rid / "native" / platform.LibName;
                        if (!File.Exists(libPath))
                        {
                            Serilog.Log.Information($"Native library missing for {currentRid}. Triggering automatic build...");
                            BuildRustForTarget(platform.Target, platform.Rid, platform.LibName, platform.RustFlags);
                        }
                    }
                }
            }

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
            var version = Version ?? "1.0.0";
            Serilog.Log.Information("Packing version {Version}...", version);

            DotNetPack(s => s
                .SetProject(LibraryProjectPath)
                .SetConfiguration(Config)
                .SetVersion(version)
                .SetNoBuild(true)
                .SetOutputDirectory(PackageOutputPath));
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
        var outputDir = NativeOutputPath / runtimeId / "native";
        Directory.CreateDirectory(outputDir);

        var env = Environment.GetEnvironmentVariables()
            .Cast<DictionaryEntry>()
            .ToDictionary(x => (string)x.Key, x => (string?)x.Value);

        if (!string.IsNullOrEmpty(rustFlags))
            env["RUSTFLAGS"] = rustFlags;

        var process = ProcessTasks.StartProcess(
            "cargo",
            $"build --release --target {target}",
            workingDirectory: RustProjectPath,
            environmentVariables: env);

        process.AssertZeroExitCode();

        var sourceLib = RustProjectPath / "target" / target / "release" / libName;
        var destLib = outputDir / libName;

        File.Copy(sourceLib, destLib, overwrite: true);
        Serilog.Log.Information("Copied {Source} → {Dest}", sourceLib, destLib);
    }
}
