# Every name this project has recovered

<table><tr>
<td valign="top">

<table>
<tr><th align="left"><code>blkops04/</code></th>
<th align="right" colspan="2">164,902 names in 6 file(s)</th>
</tr>
<tr><th align="left">asset type</th><th align="right">found here</th><th align="right">named, of all in the game</th>
</tr>
<tr><td><code>xmodel</code></td><td align="right">11,296</td><td align="right">51,419 / 61,139 &nbsp;(84.1%)</td></tr>
<tr><td><code>material</code></td><td align="right">36,135</td><td align="right">108,249 / 122,750 &nbsp;(88.2%)</td></tr>
<tr><td><code>image</code></td><td align="right">34,956</td><td align="right">141,983 / 167,360 &nbsp;(84.8%)</td></tr>
<tr><td><code>xanim</code></td><td align="right">5,545</td><td align="right">17,514 / 21,968 &nbsp;(79.7%)</td></tr>
<tr><td><code>sound_asset</code></td><td align="right">58,169</td><td align="right">66,553 / 79,263 &nbsp;(84.0%)</td></tr>
<tr><td><code>sound_alias</code></td><td align="right">18,801</td><td align="right">45,272 / 50,043 &nbsp;(90.5%)</td></tr>
</table>

</td>
<td valign="top">

<table>
<tr><th align="left"><code>blkopscw/</code></th>
<th align="right" colspan="2">71,537 names in 6 file(s)</th>
</tr>
<tr><th align="left">asset type</th><th align="right">found here</th><th align="right">named, of all in the game</th>
</tr>
<tr><td><code>xmodel</code></td><td align="right">3,726</td><td align="right">68,191 / 85,612 &nbsp;(79.7%)</td></tr>
<tr><td><code>material</code></td><td align="right">21,117</td><td align="right">141,488 / 158,158 &nbsp;(89.5%)</td></tr>
<tr><td><code>image</code></td><td align="right">10,171</td><td align="right">209,179 / 245,235 &nbsp;(85.3%)</td></tr>
<tr><td><code>xanim</code></td><td align="right">5,070</td><td align="right">21,401 / 28,468 &nbsp;(75.2%)</td></tr>
<tr><td><code>sound_asset</code></td><td align="right">1,864</td><td align="right">79,802 / 97,217 &nbsp;(82.1%)</td></tr>
<tr><td><code>sound_alias</code></td><td align="right">29,589</td><td align="right">38,008 / 50,890 &nbsp;(74.7%)</td></tr>
</table>

</td>
</tr></table>

**found here** is what this project has recovered and published in these files.
**named, of all in the game** is the whole pool: those names plus every one already in
the community tables, against every id the game holds.

They are not the same measure, and the second is much the larger.

Where `image` under `blkops04/` reads 34,956 and 141,983 / 167,360:
this project found 34,956 of the 141,983 names anybody has for that pool, and
25,377 of its ids are still nameless. The percentage is the fraction named,
not the fraction found here.

The emptiest pool is `sound_alias` under `blkopscw/`: 38,008 of 50,890 named,
so 12,882 ids carry no name at all. That is the largest unworked ground
here, and it is invisible from a count on its own.

The community half of that is measured against `cod-name-db` on 2026-08-24 and stored in
`coverage.json`, because the tables are 345 MB and are not in this repository. Names
recovered here since are added on top, which is exact rather than approximate: `submit`
drops anything the tables already publish, so a later find cannot already be counted.
What a stale baseline misses is names *somebody else* published upstream, so it
under-reports rather than over-reports. `scripts/measure_coverage.py` refreshes it.

**Generated. Do not edit anything here by hand** -- `scripts/collect_names.py` rewrites it
whenever a submission lands, and an edit would be overwritten without warning. Corrections
belong in a submission, which is the record these are built from.

One file per game and asset type, `hash,name`, sorted by name. Together they are every name
in every merged submission in `submissions/`, with duplicates removed.

## Why you might want these rather than `submissions/`

`submissions/` answers *who found what, when, and by which method* -- it is the provenance
record and the input to `scripts/methods_report.py`. It is several hundred folders, and
anybody who just wants the names has had to walk and merge them. That loop is written once,
here, and the answer committed.

These are **not** a substitute for the community tables in `cod-name-db`. Those are the
published truth and are what every search excludes against. These are this project's own
contribution to them, which is a different and smaller thing.

## Why it is split by game

The two games number their asset types differently -- `xmodel` is pool 6 in Cold War and 4 in
Black Ops 4 -- so a file mixing them mislabels every row. You can see it in the type names
themselves: both `clipmap` and `clip_map` appear, and both `localizeentry` and
`localize_entry`, because those are the two games' own names for one pool.

A name appearing under both games is not duplication. Cold War carries a great deal of Black
Ops 4's content, and a name confirmed against both games' ids is a fact about both.

Twenty-three submissions predate the game going into the folder name. They are placed by
hashing each name and asking each game's `.ids` snapshot whether it holds an asset under it
-- the same question that made the name a find. A name both snapshots hold is filed under
both, because it is genuinely a fact about both.

Only the five asset types worth searching are here. Submissions carry names for 105 types;
the rest stay in `submissions/`, which is the record.
