- #+BEGIN_QUERY
  {:title "advance exempli gratia"
  :query [
  :find (pull ?b [*])
  :where
  [?b :block/page ?p]
  [?p :page/name ?pn]
  [?b :block/marker ?marker]
  [(contains? #{"NOW" "DOING" "TODO"} ?marker)]
  ]
  }
  #+END_QUERY
-
-
- In this page we have some queries. We want to exclude the query statement from results