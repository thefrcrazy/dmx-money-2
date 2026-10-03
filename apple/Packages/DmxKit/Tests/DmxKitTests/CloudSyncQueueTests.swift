import CloudKit
import XCTest
@testable import DmxKit

final class CloudSyncQueueTests: XCTestCase {
    func testTombstoneKeepsAuthorTimestampAndClearsOldPayload() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        let id = CKRecord.ID(recordName: "transactions::synthetic", zoneID: CloudSyncBackend.zoneID)
        let record = CKRecord(recordType: "Transaction", recordID: id)
        record["payload"] = "old synthetic payload"
        let deletion = SyncChange(seq: 7, entity: "transactions", recordId: "synthetic", deleted: true,
                                  updatedAt: "2026-10-01T09:00:00.000Z", payload: nil)
        CloudSyncBackend.write(deletion, to: record)
        let decoded = try XCTUnwrap(CloudSyncBackend.remoteChange(from: record))
        XCTAssertTrue(decoded.deleted)
        XCTAssertEqual(decoded.updatedAt, deletion.updatedAt)
        XCTAssertNil(decoded.payload)
        XCTAssertEqual(record["payload"] as? String, CloudSyncBackend.tombstonePayload)
        XCTAssertNil(record["deleted"])
    }

    func testRecreationReplacesTombstoneAndLegacySaveRemainsReadable() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        let record = CKRecord(recordType: "Account", recordID: CKRecord.ID(recordName: "accounts::synthetic", zoneID: CloudSyncBackend.zoneID))
        record["deleted"] = NSNumber(value: true)
        let edit = SyncChange(seq: 8, entity: "accounts", recordId: "synthetic", deleted: false,
                              updatedAt: "2026-10-01T10:00:00.000Z", payload: "{}")
        CloudSyncBackend.write(edit, to: record)
        XCTAssertFalse(try XCTUnwrap(CloudSyncBackend.remoteChange(from: record)).deleted)
        record["deleted"] = nil
        let legacy = try XCTUnwrap(CloudSyncBackend.remoteChange(from: record))
        XCTAssertFalse(legacy.deleted)
        XCTAssertEqual(legacy.payload, "{}")
    }

    func testAcknowledgingAnOlderSaveKeepsAndReschedulesANewerEdit() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        try checkAcknowledgmentRace(sentDeleted: false, newerDeleted: false)
    }

    func testAcknowledgingADeleteDoesNotDropARecreatedRecord() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        try checkAcknowledgmentRace(sentDeleted: true, newerDeleted: false)
    }

    func testAcknowledgingASaveKeepsAPendingDeletion() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        try checkAcknowledgmentRace(sentDeleted: false, newerDeleted: true)
    }

    func testMalformedConflictNeverAcknowledgesLocalChanges() throws {
        guard #available(macOS 14.0, iOS 17.0, *) else { throw XCTSkip("CKSyncEngine requires macOS 14") }
        let id = CKRecord.ID(recordName: "accounts::synthetic", zoneID: CloudSyncBackend.zoneID)
        let local = SyncChange(seq: 12, entity: "accounts", recordId: "synthetic", deleted: false,
                               updatedAt: "2026-10-01T09:00:00.000Z", payload: "{}")
        let valid = CKRecord(recordType: "Account", recordID: id)
        CloudSyncBackend.write(SyncChange(seq: 13, entity: "accounts", recordId: "synthetic", deleted: false,
                                          updatedAt: "2026-10-01T10:00:00.000Z", payload: "{}"), to: valid)
        var malformed: [CKRecord?] = [nil]
        for field in ["entity", "recordId", "updatedAt", "payload"] {
            let missing = CKRecord(recordType: "Account", recordID: id)
            for key in ["entity", "recordId", "updatedAt", "payload"] { missing[key] = valid[key] }
            missing[field] = nil
            malformed.append(missing)
            let wrongType = CKRecord(recordType: "Account", recordID: id)
            for key in ["entity", "recordId", "updatedAt", "payload"] { wrongType[key] = valid[key] }
            wrongType[field] = NSNumber(value: 1)
            malformed.append(wrongType)
        }
        let wrongIdentity = CKRecord(recordType: "Account", recordID: id)
        CloudSyncBackend.write(SyncChange(seq: 13, entity: "accounts", recordId: "different", deleted: false,
                                          updatedAt: "2026-10-01T10:00:00.000Z", payload: "{}"), to: wrongIdentity)
        malformed.append(wrongIdentity)
        for record in malformed {
            var outgoing = [id.recordName: local]
            var acknowledged: [SyncChange] = []
            var retry: [CKSyncEngine.PendingRecordZoneChange] = []
            var serverWins: [RemoteChange] = []
            XCTAssertFalse(CloudSyncBackend.reconcileServerConflict(record, recordID: id, outgoing: &outgoing,
                                                                    acknowledged: &acknowledged, retry: &retry,
                                                                    serverWins: &serverWins))
            XCTAssertEqual(outgoing[id.recordName]?.seq, local.seq)
            XCTAssertTrue(acknowledged.isEmpty)
            XCTAssertTrue(retry.isEmpty)
            XCTAssertTrue(serverWins.isEmpty)
        }

        var outgoing = [id.recordName: local]
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        var serverWins: [RemoteChange] = []
        XCTAssertTrue(CloudSyncBackend.reconcileServerConflict(valid, recordID: id, outgoing: &outgoing,
                                                               acknowledged: &acknowledged, retry: &retry,
                                                               serverWins: &serverWins))
        XCTAssertTrue(outgoing.isEmpty)
        XCTAssertEqual(acknowledged.map(\.seq), [local.seq])
        XCTAssertEqual(serverWins.first?.updatedAt, "2026-10-01T10:00:00.000Z")
    }

    @available(macOS 14.0, iOS 17.0, *)
    private func checkAcknowledgmentRace(sentDeleted: Bool, newerDeleted: Bool) throws {
        let id = CKRecord.ID(recordName: "accounts::test", zoneID: CloudSyncBackend.zoneID)
        let first = SyncChange(seq: 1, entity: "accounts", recordId: "test", deleted: sentDeleted, updatedAt: "2026-09-10T00:00:00.000Z", payload: sentDeleted ? nil : "{}")
        let second = SyncChange(seq: 2, entity: "accounts", recordId: "test", deleted: newerDeleted, updatedAt: "2026-09-11T00:00:00.000Z", payload: newerDeleted ? nil : "{}")
        var sent = [id.recordName: first]
        var outgoing = [id.recordName: second]
        var acknowledged: [SyncChange] = []
        var retry: [CKSyncEngine.PendingRecordZoneChange] = []
        CloudSyncBackend.acknowledgeSentRecord(id, sent: &sent, outgoing: &outgoing, acknowledged: &acknowledged, retry: &retry)

        XCTAssertEqual(acknowledged.map(\.seq), [1])
        XCTAssertTrue(sent.isEmpty)
        XCTAssertEqual(outgoing[id.recordName]?.seq, 2)
        XCTAssertEqual(retry.count, 1)
        switch try XCTUnwrap(retry.first) {
        case .saveRecord(let pendingID):
            XCTAssertEqual(pendingID, id)
        case .deleteRecord(let pendingID):
            XCTFail("Tombstones must be saved, not physically deleted: \(pendingID)")
        @unknown default:
            XCTFail("Unexpected CloudKit operation")
        }
    }
}
