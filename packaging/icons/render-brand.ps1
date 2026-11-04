param([string]$Root = (Join-Path $PSScriptRoot "../.."))
$ErrorActionPreference = "Stop"
$rootPath = (Resolve-Path -LiteralPath $Root).Path
$output = Join-Path $rootPath "assets/branding"
New-Item -ItemType Directory -Force $output | Out-Null
Add-Type -AssemblyName System.Drawing

$source = @"
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;

public static class UmbraBrandRenderer
{
    static Color Hex(string value) => ColorTranslator.FromHtml(value);

    static GraphicsPath Rounded(float x, float y, float w, float h, float r)
    {
        var p = new GraphicsPath();
        p.AddArc(x, y, r * 2, r * 2, 180, 90);
        p.AddArc(x + w - r * 2, y, r * 2, r * 2, 270, 90);
        p.AddArc(x + w - r * 2, y + h - r * 2, r * 2, r * 2, 0, 90);
        p.AddArc(x, y + h - r * 2, r * 2, r * 2, 90, 90);
        p.CloseFigure();
        return p;
    }

    static void DrawMark(Graphics g, float x, float y, float size)
    {
        float s = size / 512f;
        float X(float value) => x + value * s;
        float Y(float value) => y + value * s;
        float V(float value) => value * s;

        using (var tile = Rounded(X(20), Y(20), V(472), V(472), V(124)))
        using (var fill = new LinearGradientBrush(new PointF(X(20), Y(20)), new PointF(X(492), Y(492)), Hex("#171b39"), Hex("#090d1b")))
            g.FillPath(fill, tile);

        using (var orbit = new GraphicsPath())
        using (var brush = new LinearGradientBrush(new PointF(X(72), Y(335)), new PointF(X(438), Y(160)), Hex("#7258d9"), Hex("#76ddff")))
        using (var pen = new Pen(brush, V(17)))
        {
            orbit.AddBezier(X(83), Y(335), X(101), Y(207), X(218), Y(112), X(357), Y(143));
            orbit.AddBezier(X(357), Y(143), X(391), Y(151), X(418), Y(166), X(438), Y(186));
            pen.StartCap = LineCap.Round;
            pen.EndCap = LineCap.Round;
            g.DrawPath(pen, orbit);
        }

        using (var orbit = new GraphicsPath())
        using (var brush = new LinearGradientBrush(new PointF(X(72), Y(307)), new PointF(X(442), Y(320)), Hex("#7258d9"), Hex("#76ddff")))
        using (var pen = new Pen(brush, V(13)))
        {
            orbit.AddBezier(X(72), Y(307), X(130), Y(374), X(247), Y(411), X(358), Y(377));
            orbit.AddBezier(X(358), Y(377), X(394), Y(366), X(424), Y(345), X(442), Y(317));
            pen.StartCap = LineCap.Round;
            pen.EndCap = LineCap.Round;
            g.DrawPath(pen, orbit);
        }

        using (var moon = new LinearGradientBrush(new PointF(X(160), Y(150)), new PointF(X(396), Y(366)), Hex("#d3b4ff"), Hex("#62d4ff")))
            g.FillEllipse(moon, X(160), Y(133), V(232), V(232));
        using (var shadow = new LinearGradientBrush(new PointF(X(128), Y(145)), new PointF(X(344), Y(360)), Hex("#171b35"), Hex("#0d1226")))
            g.FillEllipse(shadow, X(128), Y(147), V(216), V(216));

        using (var highlight = new Pen(Color.FromArgb(185, 227, 213, 255), V(5)))
        {
            highlight.StartCap = LineCap.Round;
            highlight.EndCap = LineCap.Round;
            g.DrawArc(highlight, X(207), Y(145), V(184), V(210), 301, 54);
        }

        using (var pixel = new SolidBrush(Hex("#c8b6ff")))
        using (var muted = new SolidBrush(Color.FromArgb(210, Hex("#c8b6ff"))))
        using (var ice = new SolidBrush(Hex("#73dfff")))
        {
            g.FillRectangle(pixel, X(374), Y(92), V(18), V(18));
            g.FillRectangle(muted, X(411), Y(126), V(11), V(11));
            g.FillRectangle(muted, X(105), Y(163), V(12), V(12));
            g.FillRectangle(pixel, X(129), Y(384), V(19), V(19));
            g.FillRectangle(ice, X(389), Y(364), V(12), V(12));
        }
    }

    static void DrawClientBadge(Graphics g)
    {
        using (var moon = new LinearGradientBrush(new PointF(2, 3), new PointF(13, 13), Hex("#c8b6ff"), Hex("#62d4ff")))
            g.FillEllipse(moon, 3, 2, 10, 10);

        var compositing = g.CompositingMode;
        g.CompositingMode = CompositingMode.SourceCopy;
        using (var clear = new SolidBrush(Color.Transparent))
            g.FillEllipse(clear, 7, 1, 8, 9);
        g.CompositingMode = compositing;

        using (var orbit = new GraphicsPath())
        using (var brush = new LinearGradientBrush(new PointF(1, 4), new PointF(15, 12), Hex("#7258d9"), Hex("#76ddff")))
        using (var pen = new Pen(brush, 1.25f))
        {
            orbit.AddBezier(1, 10, 3, 2, 11, 1, 15, 6);
            orbit.AddBezier(15, 6, 12, 9, 6, 14, 1, 10);
            pen.StartCap = LineCap.Round;
            pen.EndCap = LineCap.Round;
            g.DrawPath(pen, orbit);
        }

        using (var star = new SolidBrush(Hex("#e1d5ff")))
        using (var ice = new SolidBrush(Hex("#73dfff")))
        {
            g.FillRectangle(star, 12, 1, 1, 1);
            g.FillRectangle(ice, 3, 13, 1, 1);
        }
    }

    static Bitmap NewTransparent(int width, int height, out Graphics graphics)
    {
        var bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        graphics = Graphics.FromImage(bitmap);
        graphics.Clear(Color.Transparent);
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
        graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
        graphics.CompositingQuality = CompositingQuality.HighQuality;
        graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        return bitmap;
    }

    public static void Render(string output)
    {
        using (var badge = NewTransparent(16, 16, out var graphics))
        {
            using (graphics) DrawClientBadge(graphics);
            badge.Save(System.IO.Path.Combine(output, "client-badge.png"), ImageFormat.Png);
        }

        using (var icon = NewTransparent(512, 512, out var graphics))
        {
            using (graphics) DrawMark(graphics, 0, 0, 512);
            icon.Save(System.IO.Path.Combine(output, "icon.png"), ImageFormat.Png);
        }

        using (var title = NewTransparent(1600, 512, out var graphics))
        {
            using (graphics)
            {
                DrawMark(graphics, 0, 0, 512);
                using (var font = new Font("Bahnschrift SemiBold", 184, FontStyle.Regular, GraphicsUnit.Pixel))
                using (var shadow = new SolidBrush(Color.FromArgb(190, 0, 0, 0)))
                using (var word = new LinearGradientBrush(new PointF(570, 95), new PointF(570, 300), Hex("#ffffff"), Hex("#d7d3ff")))
                {
                    graphics.DrawString("UMBRA", font, shadow, 576, 116);
                    graphics.DrawString("UMBRA", font, word, 570, 108);
                }
                using (var font = new Font("Bahnschrift SemiBold", 50, FontStyle.Regular, GraphicsUnit.Pixel))
                using (var glow = new SolidBrush(Color.FromArgb(145, 0, 0, 0)))
                using (var label = new SolidBrush(Hex("#aa8cff")))
                {
                    graphics.DrawString("C L I E N T", font, glow, 585, 337);
                    graphics.DrawString("C L I E N T", font, label, 581, 331);
                }
                using (var line = new LinearGradientBrush(new PointF(582, 411), new PointF(910, 411), Hex("#7258d9"), Hex("#76ddff")))
                using (var pen = new Pen(line, 4))
                {
                    pen.StartCap = LineCap.Round;
                    pen.EndCap = LineCap.Round;
                    graphics.DrawLine(pen, 584, 414, 930, 414);
                }
            }
            title.Save(System.IO.Path.Combine(output, "title.png"), ImageFormat.Png);
        }
    }
}
"@

$drawingCommon = [System.Drawing.Bitmap].Assembly.Location
$drawingAssemblies = @(
    $drawingCommon,
    [System.Drawing.Color].Assembly.Location,
    (Join-Path (Split-Path -Parent $drawingCommon) "System.Private.Windows.GdiPlus.dll"),
    (Join-Path (Split-Path -Parent $drawingCommon) "System.Private.Windows.Core.dll")
)
Add-Type -TypeDefinition $source -ReferencedAssemblies $drawingAssemblies
[UmbraBrandRenderer]::Render($output)
Write-Host "Wrote Umbra icon and title art to $output"
