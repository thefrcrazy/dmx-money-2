// Vérification d'intégrité locale. Aucune politique de confiance Windows n'est remplacée.
// CryptMsgControl vérifie la signature CMS avec la clé du certificat épinglé ; le SIP Windows
// recalcule le digest Authenticode du PE. Références : docs/release.md, mode de développement.
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace DmxMoney
{
    public static class DevelopmentAuthenticode
    {
        const uint Encoding = 0x10001; // X509_ASN_ENCODING | PKCS_7_ASN_ENCODING
        const string IndirectDataOid = "1.3.6.1.4.1.311.2.1.4";
        const string Sha256Oid = "2.16.840.1.101.3.4.2.1";

        [StructLayout(LayoutKind.Sequential)] struct Blob { public uint Size; public IntPtr Data; }
        [StructLayout(LayoutKind.Sequential)] struct Algorithm { public IntPtr Oid; public Blob Parameters; }
        [StructLayout(LayoutKind.Sequential)] struct Attribute { public IntPtr Oid; public Blob Value; }
        [StructLayout(LayoutKind.Sequential)] struct IndirectData
        { public Attribute Data; public Algorithm DigestAlgorithm; public Blob Digest; }
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct SubjectInfo
        {
            public uint Size; public IntPtr SubjectType; public IntPtr File;
            public string FileName; public string DisplayName;
            public uint Reserved1; public uint IntVersion; public IntPtr Provider;
            public Algorithm DigestAlgorithm;
            public uint Flags; public uint EncodingType; public uint Reserved2;
            public uint CapiSettings; public uint SecuritySettings; public uint Index; public uint UnionChoice;
            public IntPtr AdditionalInfo; public IntPtr ClientData;
        }
        [StructLayout(LayoutKind.Sequential)] struct VerifySignature
        { public uint Size; public IntPtr Provider; public uint SignerIndex; public uint SignerType; public IntPtr Signer; }
        [StructLayout(LayoutKind.Sequential)] struct CertificateContext
        { public uint EncodingType; public IntPtr Encoded; public uint Length; public IntPtr Info; public IntPtr Store; }
        [StructLayout(LayoutKind.Sequential)] struct Attributes { public uint Count; public IntPtr Values; }
        [StructLayout(LayoutKind.Sequential)] struct CmsAttribute { public IntPtr Oid; public uint Count; public IntPtr Values; }

        [DllImport("crypt32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptSIPRetrieveSubjectGuid(string path, IntPtr file, out Guid subject);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptSIPGetSignedDataMsg(ref SubjectInfo subject, out uint encoding, uint index,
            ref uint length, [Out] byte[] data);
        [DllImport("crypt32.dll", SetLastError = true)]
        static extern IntPtr CryptMsgOpenToDecode(uint encoding, uint flags, uint type,
            IntPtr provider, IntPtr recipient, IntPtr stream);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgUpdate(IntPtr message, byte[] data, uint length, [MarshalAs(UnmanagedType.Bool)] bool final);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgGetParam(IntPtr message, uint parameter, uint index, [Out] byte[] data, ref uint length);
        [DllImport("crypt32.dll", EntryPoint = "CryptMsgGetParam", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgGetStructure(IntPtr message, uint parameter, uint index, IntPtr data, ref uint length);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgGetAndVerifySigner(IntPtr message, uint storeCount, IntPtr stores,
            uint flags, out IntPtr signer, ref uint signerIndex);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgControl(IntPtr message, uint flags, uint control, ref VerifySignature parameter);
        [DllImport("crypt32.dll", CharSet = CharSet.Ansi, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptDecodeObjectEx(uint encoding, string type, byte[] data, uint length,
            uint flags, IntPtr parameters, out IntPtr decoded, ref uint decodedLength);
        [DllImport("crypt32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptSIPVerifyIndirectData(ref SubjectInfo subject, IntPtr indirectData);
        [DllImport("crypt32.dll")] [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptMsgClose(IntPtr message);
        [DllImport("crypt32.dll")] [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CertFreeCertificateContext(IntPtr certificate);
        [DllImport("crypt32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CryptVerifyTimeStampSignature(byte[] timestamp, uint timestampLength, byte[] data,
            uint dataLength, IntPtr additionalStore, out IntPtr context, out IntPtr signer, out IntPtr store);
        [DllImport("crypt32.dll")] static extern void CryptMemFree(IntPtr pointer);
        [DllImport("crypt32.dll")] [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool CertCloseStore(IntPtr store, uint flags);
        [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr pointer);

        static void Require(bool success, string operation)
        {
            if (!success) throw new CryptographicException(operation + " : " +
                new Win32Exception(Marshal.GetLastWin32Error()).Message);
        }
        static byte[] ReadParameter(IntPtr message, uint parameter)
        {
            uint size = 0;
            Require(CryptMsgGetParam(message, parameter, 0, null, ref size), "Lecture CMS");
            if (size == 0 || size > 16 * 1024 * 1024) throw new CryptographicException("Taille CMS invalide.");
            var bytes = new byte[size];
            Require(CryptMsgGetParam(message, parameter, 0, bytes, ref size), "Lecture CMS");
            return bytes;
        }

        static void VerifyTimestamp(IntPtr message)
        {
            uint length = 0;
            Require(CryptMsgGetStructure(message, 10, 0, IntPtr.Zero, ref length), "Attributs CMS");
            if (length < Marshal.SizeOf<Attributes>() || length > 16 * 1024 * 1024)
                throw new CryptographicException("Taille des attributs CMS invalide.");
            var pointer = Marshal.AllocHGlobal((int)length);
            try
            {
                Require(CryptMsgGetStructure(message, 10, 0, pointer, ref length), "Attributs CMS");
                var attributes = Marshal.PtrToStructure<Attributes>(pointer);
                for (int i = 0; i < attributes.Count; i++)
                {
                    var attribute = Marshal.PtrToStructure<CmsAttribute>(IntPtr.Add(attributes.Values, i * Marshal.SizeOf<CmsAttribute>()));
                    if (Marshal.PtrToStringAnsi(attribute.Oid) != "1.3.6.1.4.1.311.3.3.1") continue; // RFC 3161
                    if (attribute.Count != 1) throw new CryptographicException("Horodatage RFC 3161 ambigu.");
                    var blob = Marshal.PtrToStructure<Blob>(attribute.Values);
                    var timestamp = new byte[blob.Size];
                    Marshal.Copy(blob.Data, timestamp, 0, timestamp.Length);
                    var signedBytes = ReadParameter(message, 27); // CMSG_ENCRYPTED_DIGEST
                    IntPtr context = IntPtr.Zero, signer = IntPtr.Zero, store = IntPtr.Zero;
                    try
                    {
                        Require(CryptVerifyTimeStampSignature(timestamp, (uint)timestamp.Length, signedBytes,
                            (uint)signedBytes.Length, IntPtr.Zero, out context, out signer, out store), "Horodatage RFC 3161 incorrect");
                        return;
                    }
                    finally
                    {
                        if (context != IntPtr.Zero) CryptMemFree(context);
                        if (signer != IntPtr.Zero) CertFreeCertificateContext(signer);
                        if (store != IntPtr.Zero) CertCloseStore(store, 0);
                    }
                }
                throw new CryptographicException("Horodatage Authenticode absent.");
            }
            finally { Marshal.FreeHGlobal(pointer); }
        }

        public static void VerifyFile(string path, X509Certificate2 certificate, bool requireTimestamp = true)
        {
            if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) throw new PlatformNotSupportedException();
            // Verrouillage contre les écritures pendant l'extraction de la signature et le calcul du digest.
            using (var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            {
                var handle = file.SafeFileHandle.DangerousGetHandle();
                Guid subjectGuid;
                Require(CryptSIPRetrieveSubjectGuid(path, handle, out subjectGuid), "Format Authenticode");
                var guidPointer = Marshal.AllocHGlobal(Marshal.SizeOf<Guid>());
                IntPtr message = IntPtr.Zero, indirectPointer = IntPtr.Zero, signerPointer = IntPtr.Zero;
                try
                {
                    Marshal.StructureToPtr(subjectGuid, guidPointer, false);
                    var subject = new SubjectInfo {
                        Size = (uint)Marshal.SizeOf<SubjectInfo>(), SubjectType = guidPointer,
                        File = handle, FileName = path, EncodingType = Encoding
                    };
                    uint signatureLength = 0, encoding;
                    Require(CryptSIPGetSignedDataMsg(ref subject, out encoding, 0, ref signatureLength, null), "Signature embarquée");
                    if (signatureLength == 0 || signatureLength > 16 * 1024 * 1024)
                        throw new CryptographicException("Taille de signature invalide.");
                    var signature = new byte[signatureLength];
                    Require(CryptSIPGetSignedDataMsg(ref subject, out encoding, 0, ref signatureLength, signature), "Signature embarquée");
                    // Le SIP fournit un ContentInfo PKCS#7 complet. Un type non nul attendrait
                    // le contenu nu du SignedData et refuserait son enveloppe ASN.1.
                    message = CryptMsgOpenToDecode(encoding, 0, 0, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
                    Require(message != IntPtr.Zero, "Décodage CMS");
                    Require(CryptMsgUpdate(message, signature, signatureLength, true), "Décodage CMS");
                    if (BitConverter.ToUInt32(ReadParameter(message, 5), 0) != 1) // CMSG_SIGNER_COUNT_PARAM
                        throw new CryptographicException("Un unique signataire de développement est requis.");
                    uint signerIndex = 0;
                    // Recherche dans les certificats du CMS uniquement ; aucun catalogue ou trust store ajouté.
                    Require(CryptMsgGetAndVerifySigner(message, 0, IntPtr.Zero, 0, out signerPointer, ref signerIndex), "Signature CMS embarquée");
                    var signerContext = Marshal.PtrToStructure<CertificateContext>(signerPointer);
                    var signerDer = new byte[signerContext.Length];
                    Marshal.Copy(signerContext.Encoded, signerDer, 0, signerDer.Length);
                    if (!CryptographicOperations.FixedTimeEquals(signerDer, certificate.RawData))
                        throw new CryptographicException("Signature CMS épinglée : certificat différent du certificat épinglé.");
                    var verification = new VerifySignature {
                        Size = (uint)Marshal.SizeOf<VerifySignature>(), SignerIndex = 0,
                        SignerType = 2, Signer = certificate.Handle // CMSG_VERIFY_SIGNER_CERT
                    };
                    // Vérifie la signature ET le digest du contenu CMS, sans demander une chaîne approuvée.
                    Require(CryptMsgControl(message, 0, 19, ref verification), "Signature CMS épinglée");
                    var contentType = System.Text.Encoding.ASCII.GetString(ReadParameter(message, 4)).TrimEnd('\0');
                    if (contentType != IndirectDataOid) throw new CryptographicException("Contenu Authenticode inattendu.");
                    var content = ReadParameter(message, 2); // CMSG_CONTENT_PARAM
                    uint indirectLength = 0;
                    Require(CryptDecodeObjectEx(Encoding, IndirectDataOid, content, (uint)content.Length,
                        0x8000, IntPtr.Zero, out indirectPointer, ref indirectLength), "Digest Authenticode"); // ALLOC_FLAG
                    var indirect = Marshal.PtrToStructure<IndirectData>(indirectPointer);
                    if (Marshal.PtrToStringAnsi(indirect.DigestAlgorithm.Oid) != Sha256Oid || indirect.Digest.Size != 32)
                        throw new CryptographicException("Le digest Authenticode doit utiliser SHA-256.");
                    subject.DigestAlgorithm = indirect.DigestAlgorithm;
                    // Ne pas recréer subject : le SIP peut y conserver dwIntVersion entre lecture et vérification.
                    Require(CryptSIPVerifyIndirectData(ref subject, indirectPointer), "Digest PE Authenticode incorrect");
                    if (requireTimestamp) VerifyTimestamp(message);
                    GC.KeepAlive(certificate);
                }
                finally
                {
                    if (indirectPointer != IntPtr.Zero) LocalFree(indirectPointer);
                    if (signerPointer != IntPtr.Zero) CertFreeCertificateContext(signerPointer);
                    if (message != IntPtr.Zero) CryptMsgClose(message);
                    Marshal.FreeHGlobal(guidPointer);
                }
            }
        }
    }
}
