import CloudKit
import XCTest
@testable import DmxKit

final class CloudSyncQueueTests: XCTestCase {
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
            XCTAssertFalse(newerDeleted)
            XCTAssertEqual(pendingID, id)
        case .deleteRecord(let pendingID):
            XCTAssertTrue(newerDeleted)
            XCTAssertEqual(pendingID, id)
        @unknown default:
            XCTFail("Unexpected CloudKit operation")
        }
    }
}
