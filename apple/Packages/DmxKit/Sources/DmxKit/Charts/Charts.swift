import SwiftUI

// Graphiques dessinés avec `Path` : identiques de macOS 10.15 à iOS 26, sans Swift Charts.

public struct ChartLine: Identifiable {
    public let id: String
    public let name: String
    public let color: Color
    public let values: [Double]
    public var dashed: Bool
    public var filled: Bool
    public var showsInLegend: Bool

    public init(id: String, name: String, color: Color, values: [Double], dashed: Bool = false, filled: Bool = true, showsInLegend: Bool = true) {
        self.id = id
        self.name = name
        self.color = color
        self.values = values
        self.dashed = dashed
        self.filled = filled
        self.showsInLegend = showsInLegend
    }
}

public struct ChartReferenceLine: Identifiable {
    public let id = UUID()
    public let value: Double
    public let color: Color
    public let dashed: Bool

    public init(value: Double, color: Color, dashed: Bool = false) {
        self.value = value
        self.color = color
        self.dashed = dashed
    }
}

public struct ChartMarker: Identifiable {
    public let id = UUID()
    public let index: Int
    public let color: Color

    public init(index: Int, color: Color) {
        self.index = index
        self.color = color
    }
}

/// Graduations « rondes » d'un axe.
public enum ChartScale {
    public static func niceTicks(minimum: Double, maximum: Double, count: Int = 5) -> [Double] {
        guard minimum.isFinite, maximum.isFinite else { return [0] }
        var low = minimum
        var high = maximum
        if low == high {
            low -= 1
            high += 1
        }
        let rawStep = (high - low) / Double(max(count - 1, 1))
        let magnitude = pow(10, floor(log10(rawStep)))
        let residual = rawStep / magnitude
        let step: Double
        if residual > 5 { step = 10 * magnitude } else if residual > 2 { step = 5 * magnitude } else if residual > 1 { step = 2 * magnitude } else { step = magnitude }
        let start = floor(low / step) * step
        let end = ceil(high / step) * step
        return stride(from: start, through: end + step * 0.5, by: step).map { $0 }
    }

    public static func compactLabel(_ value: Double) -> String {
        let absolute = abs(value)
        if absolute >= 1_000_000 { return String(format: "%.1fM", value / 1_000_000).replacingOccurrences(of: ".", with: ",") }
        if absolute >= 10_000 { return String(format: "%.0fk", value / 1_000) }
        if absolute >= 1_000 { return String(format: "%.1fk", value / 1_000).replacingOccurrences(of: ".", with: ",") }
        return String(format: "%.0f", value)
    }
}

/// Courbes (aires ou lignes, en escalier ou non) avec lignes de référence, marqueurs et inspection.
public struct LineChart: View {
    public let lines: [ChartLine]
    public let labels: [String]
    public var stepped: Bool
    public var referenceLines: [ChartReferenceLine]
    public var markers: [ChartMarker]
    public var tooltip: ((Int) -> AnyView)?

    @State private var selectedIndex: Int?

    public init(
        lines: [ChartLine],
        labels: [String],
        stepped: Bool = false,
        referenceLines: [ChartReferenceLine] = [],
        markers: [ChartMarker] = [],
        tooltip: ((Int) -> AnyView)? = nil
    ) {
        self.lines = lines
        self.labels = labels
        self.stepped = stepped
        self.referenceLines = referenceLines
        self.markers = markers
        self.tooltip = tooltip
    }

    private var pointCount: Int {
        lines.map { $0.values.count }.max() ?? 0
    }

    private var ticks: [Double] {
        let values = lines.flatMap { $0.values } + referenceLines.map { $0.value }
        return ChartScale.niceTicks(minimum: values.min() ?? 0, maximum: values.max() ?? 0)
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            GeometryReader { proxy in
                self.plot(size: proxy.size)
            }
            legend
        }
    }

    private func plot(size: CGSize) -> some View {
        let axisWidth: CGFloat = 44
        let labelHeight: CGFloat = 18
        let plotRect = CGRect(x: axisWidth, y: 6, width: max(size.width - axisWidth - 8, 1), height: max(size.height - labelHeight - 12, 1))
        let ticks = self.ticks
        let low = ticks.first ?? 0
        let high = ticks.last ?? 1
        let span = high - low == 0 ? 1 : high - low

        let xFor: (Int) -> CGFloat = { index in
            guard self.pointCount > 1 else { return plotRect.midX }
            return plotRect.minX + plotRect.width * CGFloat(index) / CGFloat(self.pointCount - 1)
        }
        let yFor: (Double) -> CGFloat = { value in
            plotRect.maxY - plotRect.height * CGFloat((value - low) / span)
        }

        return ZStack(alignment: .topLeading) {
            ForEach(Array(ticks.enumerated()), id: \.offset) { item in
                Path { path in
                    path.move(to: CGPoint(x: plotRect.minX, y: yFor(item.element)))
                    path.addLine(to: CGPoint(x: plotRect.maxX, y: yFor(item.element)))
                }
                .stroke(Color.primary.opacity(0.08), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))

                Text(ChartScale.compactLabel(item.element))
                    .font(.system(size: 10))
                    .foregroundColor(.secondary)
                    .frame(width: axisWidth - 6, alignment: .trailing)
                    .position(x: (axisWidth - 6) / 2, y: yFor(item.element))
            }

            ForEach(markers) { marker in
                Path { path in
                    path.move(to: CGPoint(x: xFor(marker.index), y: plotRect.minY))
                    path.addLine(to: CGPoint(x: xFor(marker.index), y: plotRect.maxY))
                }
                .stroke(marker.color.opacity(0.8), style: StrokeStyle(lineWidth: 2, dash: [3, 3]))
            }

            ForEach(referenceLines) { line in
                Path { path in
                    path.move(to: CGPoint(x: plotRect.minX, y: yFor(line.value)))
                    path.addLine(to: CGPoint(x: plotRect.maxX, y: yFor(line.value)))
                }
                .stroke(line.color, style: StrokeStyle(lineWidth: 1, dash: line.dashed ? [6, 4] : []))
            }

            ForEach(lines) { line in
                if line.filled {
                    self.areaPath(values: line.values, xFor: xFor, yFor: yFor, baseline: plotRect.maxY)
                        .fill(LinearGradient(gradient: Gradient(colors: [line.color.opacity(0.35), line.color.opacity(0.02)]), startPoint: .top, endPoint: .bottom))
                }
                self.linePath(values: line.values, xFor: xFor, yFor: yFor)
                    .stroke(line.color, style: StrokeStyle(lineWidth: line.dashed ? 1.5 : 2, lineCap: .round, lineJoin: .round, dash: line.dashed ? [4, 3] : []))
            }

            ForEach(xLabelIndexes(width: plotRect.width), id: \.self) { index in
                Text(index < labels.count ? labels[index] : "")
                    .font(.system(size: 10))
                    .foregroundColor(.secondary)
                    .position(x: xFor(index), y: plotRect.maxY + labelHeight / 2 + 4)
            }

            if let index = selectedIndex, index < pointCount {
                Path { path in
                    path.move(to: CGPoint(x: xFor(index), y: plotRect.minY))
                    path.addLine(to: CGPoint(x: xFor(index), y: plotRect.maxY))
                }
                .stroke(Color.primary.opacity(0.35), lineWidth: 1)

                if let tooltip = tooltip {
                    tooltip(index)
                        .fixedWidth(220)
                        .position(x: min(max(xFor(index), plotRect.minX + 110), plotRect.maxX - 110), y: plotRect.minY + 60)
                }
            }
        }
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    guard self.pointCount > 1 else { return }
                    let ratio = (value.location.x - plotRect.minX) / plotRect.width
                    self.selectedIndex = min(max(Int((ratio * CGFloat(self.pointCount - 1)).rounded()), 0), self.pointCount - 1)
                }
                .onEnded { _ in
                    #if os(iOS)
                    self.selectedIndex = nil
                    #endif
                }
        )
    }

    private func xLabelIndexes(width: CGFloat) -> [Int] {
        guard pointCount > 0 else { return [] }
        let maximumLabels = max(Int(width / 70), 2)
        let step = max(Int(ceil(Double(pointCount) / Double(maximumLabels))), 1)
        return Array(stride(from: 0, to: pointCount, by: step))
    }

    private func linePath(values: [Double], xFor: (Int) -> CGFloat, yFor: (Double) -> CGFloat) -> Path {
        Path { path in
            for (index, value) in values.enumerated() {
                let point = CGPoint(x: xFor(index), y: yFor(value))
                if index == 0 {
                    path.move(to: point)
                } else if stepped {
                    path.addLine(to: CGPoint(x: point.x, y: yFor(values[index - 1])))
                    path.addLine(to: point)
                } else {
                    path.addLine(to: point)
                }
            }
        }
    }

    private func areaPath(values: [Double], xFor: (Int) -> CGFloat, yFor: (Double) -> CGFloat, baseline: CGFloat) -> Path {
        var path = linePath(values: values, xFor: xFor, yFor: yFor)
        guard !values.isEmpty else { return path }
        path.addLine(to: CGPoint(x: xFor(values.count - 1), y: baseline))
        path.addLine(to: CGPoint(x: xFor(0), y: baseline))
        path.closeSubpath()
        return path
    }

    private var legend: some View {
        HStack(spacing: 14) {
            ForEach(lines.filter { $0.showsInLegend }) { line in
                HStack(spacing: 6) {
                    RoundedRectangle(cornerRadius: 2).fill(line.color).frame(width: 14, height: 3)
                    Text(line.name).font(.system(size: 11)).foregroundColor(.secondary)
                }
            }
        }
    }
}

public struct BarGroup: Identifiable {
    public let id: Int
    public let label: String
    public let values: [Double]

    public init(id: Int, label: String, values: [Double]) {
        self.id = id
        self.label = label
        self.values = values
    }
}

/// Barres groupées (ex. revenus et dépenses par période).
public struct BarChart: View {
    public let groups: [BarGroup]
    public let colors: [Color]
    public let names: [String]
    public var tooltip: ((Int) -> AnyView)?

    @State private var selectedIndex: Int?

    public init(groups: [BarGroup], colors: [Color], names: [String], tooltip: ((Int) -> AnyView)? = nil) {
        self.groups = groups
        self.colors = colors
        self.names = names
        self.tooltip = tooltip
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            GeometryReader { proxy in
                self.plot(size: proxy.size)
            }
            HStack(spacing: 14) {
                ForEach(Array(names.enumerated()), id: \.offset) { item in
                    HStack(spacing: 6) {
                        RoundedRectangle(cornerRadius: 2).fill(self.colors[item.offset % self.colors.count]).frame(width: 10, height: 10)
                        Text(item.element).font(.system(size: 11)).foregroundColor(.secondary)
                    }
                }
            }
        }
    }

    private func plot(size: CGSize) -> some View {
        let axisWidth: CGFloat = 44
        let labelHeight: CGFloat = 18
        let plotRect = CGRect(x: axisWidth, y: 6, width: max(size.width - axisWidth - 8, 1), height: max(size.height - labelHeight - 12, 1))
        let maximum = groups.flatMap { $0.values }.max() ?? 0
        let ticks = ChartScale.niceTicks(minimum: 0, maximum: max(maximum, 1))
        let high = ticks.last ?? 1
        let slot = plotRect.width / CGFloat(max(groups.count, 1))
        let seriesCount = max(groups.first?.values.count ?? 1, 1)
        let barWidth = max(min(slot * 0.8 / CGFloat(seriesCount), 28), 2)
        let labelStep = max(Int(ceil(Double(groups.count) / Double(max(Int(plotRect.width / 60), 1)))), 1)

        return ZStack(alignment: .topLeading) {
            ForEach(Array(ticks.enumerated()), id: \.offset) { item in
                Path { path in
                    let y = plotRect.maxY - plotRect.height * CGFloat(item.element / high)
                    path.move(to: CGPoint(x: plotRect.minX, y: y))
                    path.addLine(to: CGPoint(x: plotRect.maxX, y: y))
                }
                .stroke(Color.primary.opacity(0.08), style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
                Text(ChartScale.compactLabel(item.element))
                    .font(.system(size: 10))
                    .foregroundColor(.secondary)
                    .frame(width: axisWidth - 6, alignment: .trailing)
                    .position(x: (axisWidth - 6) / 2, y: plotRect.maxY - plotRect.height * CGFloat(item.element / high))
            }

            ForEach(groups) { group in
                ForEach(Array(group.values.enumerated()), id: \.offset) { item in
                    let height = plotRect.height * CGFloat(item.element / high)
                    let groupStart = plotRect.minX + slot * CGFloat(group.id) + (slot - barWidth * CGFloat(seriesCount)) / 2
                    RoundedRectangle(cornerRadius: min(4, barWidth / 2))
                        .fill(self.colors[item.offset % self.colors.count].opacity(self.selectedIndex == nil || self.selectedIndex == group.id ? 1 : 0.45))
                        .frame(width: barWidth, height: max(height, 0))
                        .position(x: groupStart + barWidth * (CGFloat(item.offset) + 0.5), y: plotRect.maxY - height / 2)
                }
                if group.id % labelStep == 0 {
                    Text(group.label)
                        .font(.system(size: 10))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                        .position(x: plotRect.minX + slot * (CGFloat(group.id) + 0.5), y: plotRect.maxY + labelHeight / 2 + 4)
                }
            }

            if let index = selectedIndex, let tooltip = tooltip, index < groups.count {
                tooltip(index)
                    .fixedWidth(200)
                    .position(x: min(max(plotRect.minX + slot * (CGFloat(index) + 0.5), plotRect.minX + 100), plotRect.maxX - 100), y: plotRect.minY + 50)
            }
        }
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    guard !self.groups.isEmpty else { return }
                    let index = Int((value.location.x - plotRect.minX) / slot)
                    self.selectedIndex = min(max(index, 0), self.groups.count - 1)
                }
                .onEnded { _ in
                    #if os(iOS)
                    self.selectedIndex = nil
                    #endif
                }
        )
    }
}

public struct DonutSlice: Identifiable {
    public let id: String
    public let label: String
    public let value: Double
    public let color: Color
    public let hidden: Bool

    public init(id: String, label: String, value: Double, color: Color, hidden: Bool = false) {
        self.id = id
        self.label = label
        self.value = value
        self.color = color
        self.hidden = hidden
    }
}

/// Anneau de répartition ; les parts masquées ne comptent pas dans le total.
public struct DonutChart<Center: View>: View {
    public let slices: [DonutSlice]
    public var thickness: CGFloat
    public let center: Center

    public init(slices: [DonutSlice], thickness: CGFloat = 18, @ViewBuilder center: () -> Center) {
        self.slices = slices
        self.thickness = thickness
        self.center = center()
    }

    public var body: some View {
        GeometryReader { proxy in
            ZStack {
                Circle()
                    .stroke(Color.primary.opacity(0.07), lineWidth: self.thickness)
                    .padding(self.thickness / 2)
                ForEach(self.segments()) { segment in
                    DonutArc(start: segment.start, end: segment.end)
                        .stroke(segment.color, style: StrokeStyle(lineWidth: self.thickness, lineCap: .butt))
                        .padding(self.thickness / 2)
                }
                self.center
            }
            .frame(width: min(proxy.size.width, proxy.size.height), height: min(proxy.size.width, proxy.size.height))
            .position(x: proxy.size.width / 2, y: proxy.size.height / 2)
        }
    }

    private struct Segment: Identifiable {
        let id: String
        let start: Double
        let end: Double
        let color: Color
    }

    private func segments() -> [Segment] {
        let visible = slices.filter { !$0.hidden && $0.value > 0 }
        let total = visible.reduce(0) { $0 + $1.value }
        guard total > 0 else { return [] }
        var cursor = 0.0
        let gap = visible.count > 1 ? 0.004 : 0
        return visible.map { slice in
            let start = cursor
            cursor += slice.value / total
            return Segment(id: slice.id, start: start + gap, end: max(cursor - gap, start + gap), color: slice.color)
        }
    }
}

struct DonutArc: Shape {
    let start: Double
    let end: Double

    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.addArc(
            center: CGPoint(x: rect.midX, y: rect.midY),
            radius: min(rect.width, rect.height) / 2,
            startAngle: .degrees(start * 360 - 90),
            endAngle: .degrees(end * 360 - 90),
            clockwise: false
        )
        return path
    }
}

/// Barre de progression (budget) compatible macOS 10.15.
public struct ProgressBar: View {
    public let progress: Double
    public let color: Color
    public var height: CGFloat

    public init(progress: Double, color: Color, height: CGFloat = 6) {
        self.progress = progress
        self.color = color
        self.height = height
    }

    public var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                Capsule().fill(Color.primary.opacity(0.08))
                Capsule()
                    .fill(self.color)
                    .frame(width: proxy.size.width * CGFloat(min(max(self.progress / 100, 0), 1)))
            }
        }
        .frame(height: height)
    }
}

extension View {
    func fixedWidth(_ width: CGFloat) -> some View {
        frame(width: width)
    }
}
