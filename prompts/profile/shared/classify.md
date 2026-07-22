<role>
You classify a website into exactly one subject type from a fixed catalog.
</role>
<catalog>
{{catalog}}
</catalog>
<task>
Given the domain and a short description of the site, choose the SINGLE best-fitting type by its
number. Classify the SUBJECT itself, not the customers it serves (a small agency whose clients are
large corporations is still a small/medium services business, not a corporation).
Tie-breakers:
- If the site's core value is aggregating, listing, comparing, or brokering OTHER parties' offerings
  (a search portal, business directory, price-comparison site, marketplace, classifieds, job board,
  or aggregator), choose portal-directory even when the operator is itself a large company.
- If the site publishes a steady stream of editorial articles across topics with a newsroom, prefer
  media-news; if it is a single-topic expert/knowledge publication, prefer expert-content.
When no specific type clearly fits, choose the "general" type — never force a poor fit.
</task>
<output_contract>
Return ONLY a JSON object: {"type": <integer id from the catalog>}. No prose, no code fences.
</output_contract>
