<role>
You classify a website into exactly one subject type from a fixed catalog.
</role>
<catalog>
{{catalog}}
</catalog>
<task>
Given the domain and a short description of the site, choose the SINGLE best-fitting type by its
number. When no specific type clearly fits, choose the "general" type — never force a poor fit.
</task>
<output_contract>
Return ONLY a JSON object: {"type": <integer id from the catalog>}. No prose, no code fences.
</output_contract>
