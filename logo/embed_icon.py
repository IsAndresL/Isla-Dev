import os
import sys
import struct
import ctypes
from ctypes import wintypes

k32 = ctypes.windll.kernel32
BeginUpdateResourceW = k32.BeginUpdateResourceW
BeginUpdateResourceW.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
BeginUpdateResourceW.restype = wintypes.HANDLE

UpdateResourceW = k32.UpdateResourceW
UpdateResourceW.argtypes = [wintypes.HANDLE, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.WORD, ctypes.c_void_p, wintypes.DWORD]
UpdateResourceW.restype = wintypes.BOOL

EndUpdateResourceW = k32.EndUpdateResourceW
EndUpdateResourceW.argtypes = [wintypes.HANDLE, wintypes.BOOL]
EndUpdateResourceW.restype = wintypes.BOOL

RT_ICON = ctypes.cast(3, wintypes.LPCWSTR)
RT_GROUP_ICON = ctypes.cast(14, wintypes.LPCWSTR)

def embed_icon(exe_path, ico_path):
    exe_path = os.path.abspath(exe_path)
    ico_path = os.path.abspath(ico_path)
    if not os.path.exists(exe_path):
        print(f"Error: exe not found at {exe_path}")
        return False
    if not os.path.exists(ico_path):
        print(f"Error: ico not found at {ico_path}")
        return False

    with open(ico_path, 'rb') as f:
        data = f.read()

    reserved, ico_type, count = struct.unpack('<HHH', data[:6])
    if ico_type != 1:
        print("Error: Invalid ICO format")
        return False

    entries = []
    offset = 6
    for i in range(count):
        w, h, colors, res, planes, bpp, size, img_offset = struct.unpack('<BBBBHHII', data[offset:offset+16])
        entries.append((w, h, colors, res, planes, bpp, size, img_offset))
        offset += 16

    hUpdate = BeginUpdateResourceW(exe_path, False)
    if not hUpdate:
        err = k32.GetLastError()
        print(f"BeginUpdateResourceW failed: {err}")
        return False

    for i, entry in enumerate(entries):
        img_data = data[entry[7] : entry[7] + entry[6]]
        buf = (ctypes.c_char * len(img_data)).from_buffer_copy(img_data)
        res_id = ctypes.cast(i + 1, wintypes.LPCWSTR)
        ok = UpdateResourceW(hUpdate, RT_ICON, res_id, 0, buf, len(img_data))
        if not ok:
            err = k32.GetLastError()
            EndUpdateResourceW(hUpdate, True)
            print(f"UpdateResourceW (icon {i+1}) failed: {err}")
            return False

    grp = bytearray()
    grp += struct.pack('<HHH', reserved, ico_type, count)
    for i, entry in enumerate(entries):
        grp += struct.pack('<BBBBHHIH', entry[0], entry[1], entry[2], entry[3], entry[4], entry[5], entry[6], i + 1)

    grp_buf = (ctypes.c_char * len(grp)).from_buffer_copy(grp)
    grp_id = ctypes.cast(1, wintypes.LPCWSTR)
    ok = UpdateResourceW(hUpdate, RT_GROUP_ICON, grp_id, 0, grp_buf, len(grp))
    if not ok:
        err = k32.GetLastError()
        EndUpdateResourceW(hUpdate, True)
        print(f"UpdateResourceW (group icon) failed: {err}")
        return False

    if not EndUpdateResourceW(hUpdate, False):
        err = k32.GetLastError()
        print(f"EndUpdateResourceW failed: {err}")
        return False

    print(f"[OK] Icono incrustado exitosamente en: {exe_path}")
    return True

if __name__ == '__main__':
    base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ico = os.path.join(base, "logo", "app.ico")
    
    if len(sys.argv) > 1:
        target = sys.argv[1]
    else:
        target = os.path.join(base, "target", "x86_64-pc-windows-gnu", "release", "isla-dev.exe")
        
    embed_icon(target, ico)

