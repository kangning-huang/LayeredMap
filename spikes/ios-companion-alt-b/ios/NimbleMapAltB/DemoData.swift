import Foundation

/// First-run example overlay for Alternative B. Tiny world cities FeatureCollection.
enum DemoData {
    static let mapName = "World example"

    /// Natural-earth–style sample points (not a download). Enough for MapLibre + tap stub.
    static let worldCitiesGeoJSON = """
    {"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"name":"Shanghai","kind":"City","temp_c":31.2,"rank":"3rd hottest of 17"},"geometry":{"type":"Point","coordinates":[121.47,31.23]}},
      {"type":"Feature","properties":{"name":"Singapore","kind":"City","temp_c":30.1,"rank":"5th hottest of 17"},"geometry":{"type":"Point","coordinates":[103.85,1.29]}},
      {"type":"Feature","properties":{"name":"Lagos","kind":"City","temp_c":32.4,"rank":"hottest of 17"},"geometry":{"type":"Point","coordinates":[3.38,6.52]}},
      {"type":"Feature","properties":{"name":"São Paulo","kind":"City","temp_c":24.0,"rank":"11th of 17"},"geometry":{"type":"Point","coordinates":[-46.63,-23.55]}},
      {"type":"Feature","properties":{"name":"London","kind":"City","temp_c":18.5,"rank":"coolest of 17"},"geometry":{"type":"Point","coordinates":[-0.13,51.51]}}
    ]}
    """
}
