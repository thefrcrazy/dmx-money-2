using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace DmxMoney.Interop;

/// <summary>
/// Trouve <c>dmx_ffi</c> à côté de l'exécutable ou dans <c>runtimes/&lt;rid&gt;/native</c>
/// (tests sur macOS/Linux, build multi-architecture sous Windows).
/// </summary>
internal static class NativeLibraryLoader
{
    private const string LibraryName = "dmx_ffi";

    [ModuleInitializer]
    internal static void Register()
    {
        NativeLibrary.SetDllImportResolver(typeof(NativeLibraryLoader).Assembly, Resolve);
    }

    private static IntPtr Resolve(string name, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (name != LibraryName)
        {
            return IntPtr.Zero;
        }

        var fileName = OperatingSystem.IsWindows() ? "dmx_ffi.dll"
            : OperatingSystem.IsMacOS() ? "libdmx_ffi.dylib"
            : "libdmx_ffi.so";

        var baseDirectory = AppContext.BaseDirectory;
        string[] candidates =
        [
            Path.Combine(baseDirectory, fileName),
            Path.Combine(baseDirectory, "runtimes", RuntimeInformation.RuntimeIdentifier, "native", fileName),
            Path.Combine(baseDirectory, "runtimes", PortableRuntimeIdentifier(), "native", fileName),
        ];

        foreach (var candidate in candidates)
        {
            if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out var handle))
            {
                return handle;
            }
        }

        return IntPtr.Zero;
    }

    private static string PortableRuntimeIdentifier()
    {
        var os = OperatingSystem.IsWindows() ? "win" : OperatingSystem.IsMacOS() ? "osx" : "linux";
        var architecture = RuntimeInformation.ProcessArchitecture switch
        {
            Architecture.Arm64 => "arm64",
            Architecture.X86 => "x86",
            _ => "x64",
        };
        return $"{os}-{architecture}";
    }
}
