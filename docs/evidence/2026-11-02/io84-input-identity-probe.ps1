$ErrorActionPreference = 'Stop'
# Read-only inventory. No input registration, event capture, device opens or writes.
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class IoInputIdentityProbe {
    [StructLayout(LayoutKind.Sequential)] struct RawDevice { public IntPtr Handle; public uint Type; }
    [StructLayout(LayoutKind.Sequential)] struct PropertyKey { public Guid Format; public uint Id;
        public PropertyKey(string f, uint i) { Format = new Guid(f); Id = i; } }
    [DllImport("user32.dll", SetLastError=true)] static extern uint GetRawInputDeviceList(
        [Out] RawDevice[] devices, ref uint count, uint size);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern uint GetRawInputDeviceInfoW(
        IntPtr device, uint command, StringBuilder value, ref uint size);
    [DllImport("cfgmgr32.dll", CharSet=CharSet.Unicode)] static extern uint CM_Get_Device_Interface_PropertyW(
        string path, ref PropertyKey key, out uint type, byte[] value, ref uint size, uint flags);
    [DllImport("cfgmgr32.dll", CharSet=CharSet.Unicode)] static extern uint CM_Locate_DevNodeW(
        out uint node, string instance, uint flags);
    [DllImport("cfgmgr32.dll", CharSet=CharSet.Unicode)] static extern uint CM_Get_DevNode_PropertyW(
        uint node, ref PropertyKey key, out uint type, byte[] value, ref uint size, uint flags);
    public sealed class Result {
        public int AllKeyboardCollections;
        public int MatchingIoKeyboardCollections;
        public int UnresolvedIoKeyboardCollections;
        public int IoContainerGroups;
        public bool AllIoCollectionsMatchVendorContainer;
    }
    static Guid Container(string path) {
        var instanceKey = new PropertyKey("78c34fc8-104a-4aca-9ea4-524d52996e57", 256);
        uint type, size = 0;
        uint status = CM_Get_Device_Interface_PropertyW(path, ref instanceKey, out type, null, ref size, 0);
        if (status != 26 || type != 18 || size < 2 || size > 65536) throw new Exception("Interface instance query failed");
        byte[] instanceBytes = new byte[size];
        if (CM_Get_Device_Interface_PropertyW(path, ref instanceKey, out type, instanceBytes, ref size, 0) != 0 || type != 18)
            throw new Exception("Interface instance read failed");
        string instance = Encoding.Unicode.GetString(instanceBytes, 0, (int)size).TrimEnd('\0');
        uint node;
        if (CM_Locate_DevNodeW(out node, instance, 0) != 0) throw new Exception("Devnode lookup failed");
        var containerKey = new PropertyKey("8c7ed206-3f8a-4827-b3ab-ae9e1faefc6c", 2);
        byte[] guid = new byte[16]; size = 16;
        if (CM_Get_DevNode_PropertyW(node, ref containerKey, out type, guid, ref size, 0) != 0 || type != 13 || size != 16)
            throw new Exception("Container query failed");
        Guid result = new Guid(guid);
        if (result == Guid.Empty) throw new Exception("Empty container");
        return result;
    }
    public static Result Inspect(Guid expected) {
        if (expected == Guid.Empty) throw new Exception("Missing vendor container");
        uint count = 0, itemSize = (uint)Marshal.SizeOf(typeof(RawDevice));
        if (GetRawInputDeviceList(null, ref count, itemSize) == uint.MaxValue) throw new Exception("Raw input inventory failed");
        RawDevice[] devices = null; uint actual = 0;
        for (int attempt = 0; attempt < 3; attempt++) {
            if (count > 4096) throw new Exception("Unexpected inventory size");
            devices = new RawDevice[count];
            actual = GetRawInputDeviceList(devices, ref count, itemSize);
            if (actual != uint.MaxValue) break;
            if (Marshal.GetLastWin32Error() != 122) throw new Exception("Raw input inventory read failed");
        }
        if (actual == uint.MaxValue) throw new Exception("Raw input inventory kept changing");
        var result = new Result(); var containers = new HashSet<Guid>(); bool matches = true;
        for (int i = 0; i < actual; i++) {
            if (devices[i].Type != 1) continue;
            result.AllKeyboardCollections++;
            uint chars = 0;
            if (GetRawInputDeviceInfoW(devices[i].Handle, 0x20000007, null, ref chars) == uint.MaxValue || chars > 32768)
                throw new Exception("Raw keyboard name size failed");
            var name = new StringBuilder((int)chars + 1);
            if (GetRawInputDeviceInfoW(devices[i].Handle, 0x20000007, name, ref chars) == uint.MaxValue)
                throw new Exception("Raw keyboard name read failed");
            string path = name.ToString();
            if (path.IndexOf("VID_0C45&PID_80D6", StringComparison.OrdinalIgnoreCase) < 0) continue;
            result.MatchingIoKeyboardCollections++;
            try { Guid container = Container(path); containers.Add(container); matches &= container == expected; }
            catch { result.UnresolvedIoKeyboardCollections++; matches = false; }
        }
        result.IoContainerGroups = containers.Count;
        result.AllIoCollectionsMatchVendorContainer = result.MatchingIoKeyboardCollections > 0 && matches;
        return result;
    }
}
'@
$vendorNodes = @(Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'HID\VID_0C45&PID_80D6&MI_02*' })
if ($vendorNodes.Count -ne 1) { throw 'Expected exactly one IO84 vendor-control devnode' }
$vendorContainer = [Guid](Get-PnpDeviceProperty -InstanceId $vendorNodes[0].InstanceId -KeyName 'DEVPKEY_Device_ContainerId').Data
[IoInputIdentityProbe]::Inspect($vendorContainer) | ConvertTo-Json
