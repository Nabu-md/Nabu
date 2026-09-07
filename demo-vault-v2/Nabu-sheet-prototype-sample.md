---
type: Note
_display: sheet
tags:
  - spreadsheet
_sheet:
  frozen_rows: 1
  columns:
    A:
      width: 180
    B:
      width: 140
    C:
      width: 140
    D:
      width: 140
    E:
      width: 140
  cells:
    A2:
      bold: true
    A5:
      bold: true
---
Metric,January,February,March,Quarter
Subscriptions,1200,1350,1500,=SUM(B2:D2)
Services,800,900,700,=SUM(B3:D3)
Expenses,650,700,760,=SUM(B4:D4)
Net,=B2+B3-B4,=C2+C3-C4,=D2+D3-D4,=SUM(B5:D5)
Growth,,=(C5-B5)/B5,=(D5-C5)/C5,=(E5-B5)/B5
