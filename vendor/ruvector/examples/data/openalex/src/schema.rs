//! OpenAlex entity schemas
//!
//! Represents the core entity types from OpenAlex:
//! - Works (publications)
//! - Authors
//! - Institutions
//! - Topics/Concepts
//! - Funders
//! - Sources (journals, conferences)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A scholarly work (paper, book, dataset, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    /// OpenAlex ID (e.g., "W2741809807")
    pub id: String,

    /// DOI (if available)
    pub doi: Option<String>,

    /// Work title
    pub title: String,

    /// Publication date
    pub publication_date: Option<DateTime<Utc>>,

    /// Publication year
    pub publication_year: Option<i32>,

    /// Work type (article, book, dataset, etc.)
    #[serde(rename = "type")]
    pub work_type: Option<String>,

    /// Open access status
    pub open_access: Option<OpenAccessStatus>,

    /// Citation count
    pub cited_by_count: u64,

    /// Authors and their affiliations
    #[serde(default)]
    pub authorships: Vec<Authorship>,

    /// Primary topic
    pub primary_topic: Option<TopicReference>,

    /// All associated topics
    #[serde(default)]
    pub topics: Vec<TopicReference>,

    /// Legacy concepts (deprecated but still in API)
    #[serde(default)]
    pub concepts: Vec<ConceptReference>,

    /// Referenced works (citations)
    #[serde(default)]
    pub referenced_works: Vec<String>,

    /// Related works
    #[serde(default)]
    pub related_works: Vec<String>,

    /// Abstract (inverted index format in API)
    pub abstract_inverted_index: Option<serde_json::Value>,

    /// Publication venue
    pub primary_location: Option<Location>,

    /// Grants/funding
    #[serde(default)]
    pub grants: Vec<Grant>,

    /// Bibliographic info
    pub biblio: Option<Biblio>,

    /// Last update time
    pub updated_date: Option<DateTime<Utc>>,
}

/// Open access status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAccessStatus {
    /// Is this work open access?
    pub is_oa: bool,

    /// OA status type (gold, green, hybrid, bronze)
    pub oa_status: Option<String>,

    /// OA URL if available
    pub oa_url: Option<String>,
}

/// Author and affiliation information for a work
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authorship {
    /// Author position (first, middle, last)
    pub author_position: AuthorPosition,

    /// Author details
    pub author: AuthorReference,

    /// Institutions at time of publication
    #[serde(default)]
    pub institutions: Vec<InstitutionReference>,

    /// Countries
    #[serde(default)]
    pub countries: Vec<String>,

    /// Is corresponding author
    #[serde(default)]
    pub is_corresponding: bool,

    /// Raw affiliation string
    pub raw_affiliation_string: Option<String>,
}

/// Author position in author list
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthorPosition {
    /// First author
    First,
    /// Middle author
    Middle,
    /// Last author
    Last,
}

/// Reference to an author
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorReference {
    /// OpenAlex author ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// ORCID (if available)
    pub orcid: Option<String>,
}

/// Reference to an institution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstitutionReference {
    /// OpenAlex institution ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Institution type (education, company, etc.)
    #[serde(rename = "type")]
    pub institution_type: Option<String>,

    /// Country code
    pub country_code: Option<String>,

    /// ROR ID
    pub ror: Option<String>,
}

/// Reference to a topic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicReference {
    /// OpenAlex topic ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Relevance score (0-1)
    #[serde(default)]
    pub score: f64,

    /// Subfield
    pub subfield: Option<FieldReference>,

    /// Field
    pub field: Option<FieldReference>,

    /// Domain
    pub domain: Option<FieldReference>,
}

/// Reference to a concept (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptReference {
    /// OpenAlex concept ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Wikidata ID
    pub wikidata: Option<String>,

    /// Relevance score
    #[serde(default)]
    pub score: f64,

    /// Hierarchy level (0 = root)
    #[serde(default)]
    pub level: u32,
}

/// Reference to a field/domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldReference {
    /// OpenAlex ID
    pub id: String,

    /// Display name
    pub display_name: String,
}

/// Publication location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    /// Is primary location
    #[serde(default)]
    pub is_primary: bool,

    /// Landing page URL
    pub landing_page_url: Option<String>,

    /// PDF URL
    pub pdf_url: Option<String>,

    /// Source (journal/conference)
    pub source: Option<SourceReference>,

    /// License
    pub license: Option<String>,

    /// Version
    pub version: Option<String>,
}

/// Reference to a source (journal, conference, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    /// OpenAlex source ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// ISSN
    pub issn_l: Option<String>,

    /// Source type
    #[serde(rename = "type")]
    pub source_type: Option<String>,

    /// Is Open Access journal
    #[serde(default)]
    pub is_oa: bool,

    /// Host organization
    pub host_organization: Option<String>,
}

/// Grant/funding information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    /// Funder
    pub funder: Option<FunderReference>,

    /// Funder display name
    pub funder_display_name: Option<String>,

    /// Award ID
    pub award_id: Option<String>,
}

/// Reference to a funder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunderReference {
    /// OpenAlex funder ID
    pub id: String,

    /// Display name
    pub display_name: String,
}

/// Bibliographic details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biblio {
    /// Volume
    pub volume: Option<String>,

    /// Issue
    pub issue: Option<String>,

    /// First page
    pub first_page: Option<String>,

    /// Last page
    pub last_page: Option<String>,
}

/// Full author entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    /// OpenAlex author ID
    pub id: String,

    /// ORCID
    pub orcid: Option<String>,

    /// Display name
    pub display_name: String,

    /// Alternative names
    #[serde(default)]
    pub display_name_alternatives: Vec<String>,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// H-index
    pub summary_stats: Option<AuthorStats>,

    /// Most recent institution
    pub last_known_institution: Option<InstitutionReference>,

    /// All affiliations
    #[serde(default)]
    pub affiliations: Vec<Affiliation>,

    /// Topic areas
    #[serde(default)]
    pub topics: Vec<TopicReference>,

    /// Works API URL
    pub works_api_url: Option<String>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

/// Author summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorStats {
    /// H-index
    pub h_index: Option<u32>,

    /// i10-index
    pub i10_index: Option<u32>,

    /// Two-year mean citedness
    #[serde(rename = "2yr_mean_citedness")]
    pub two_year_mean_citedness: Option<f64>,
}

/// Author affiliation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Affiliation {
    /// Institution
    pub institution: InstitutionReference,

    /// Years affiliated
    #[serde(default)]
    pub years: Vec<i32>,
}

/// Full institution entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Institution {
    /// OpenAlex institution ID
    pub id: String,

    /// ROR ID
    pub ror: Option<String>,

    /// Display name
    pub display_name: String,

    /// Country code
    pub country_code: Option<String>,

    /// Institution type
    #[serde(rename = "type")]
    pub institution_type: Option<String>,

    /// Homepage URL
    pub homepage_url: Option<String>,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// Geographic info
    pub geo: Option<GeoLocation>,

    /// Parent institutions
    #[serde(default)]
    pub lineage: Vec<String>,

    /// Associated institutions
    #[serde(default)]
    pub associated_institutions: Vec<InstitutionReference>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

/// Geographic location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    /// City
    pub city: Option<String>,

    /// Region/state
    pub region: Option<String>,

    /// Country
    pub country: Option<String>,

    /// Country code
    pub country_code: Option<String>,

    /// Latitude
    pub latitude: Option<f64>,

    /// Longitude
    pub longitude: Option<f64>,
}

/// Full topic entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
    /// OpenAlex topic ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Description
    pub description: Option<String>,

    /// Keywords
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// Subfield
    pub subfield: Option<FieldReference>,

    /// Field
    pub field: Option<FieldReference>,

    /// Domain
    pub domain: Option<FieldReference>,

    /// Sibling topics
    #[serde(default)]
    pub siblings: Vec<TopicReference>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

/// Legacy concept entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// OpenAlex concept ID
    pub id: String,

    /// Wikidata ID
    pub wikidata: Option<String>,

    /// Display name
    pub display_name: String,

    /// Description
    pub description: Option<String>,

    /// Hierarchy level
    pub level: u32,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// Parent concepts
    #[serde(default)]
    pub ancestors: Vec<ConceptReference>,

    /// Child concepts
    #[serde(default)]
    pub related_concepts: Vec<ConceptReference>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

/// Full source entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// OpenAlex source ID
    pub id: String,

    /// ISSN-L
    pub issn_l: Option<String>,

    /// All ISSNs
    #[serde(default)]
    pub issn: Vec<String>,

    /// Display name
    pub display_name: String,

    /// Publisher
    pub host_organization: Option<String>,

    /// Source type (journal, conference, etc.)
    #[serde(rename = "type")]
    pub source_type: Option<String>,

    /// Is Open Access
    #[serde(default)]
    pub is_oa: bool,

    /// Homepage URL
    pub homepage_url: Option<String>,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// Topics
    #[serde(default)]
    pub topics: Vec<TopicReference>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

/// Full funder entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funder {
    /// OpenAlex funder ID
    pub id: String,

    /// Display name
    pub display_name: String,

    /// Alternative names
    #[serde(default)]
    pub alternate_titles: Vec<String>,

    /// Country code
    pub country_code: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Homepage URL
    pub homepage_url: Option<String>,

    /// Grants count
    pub grants_count: u64,

    /// Works count
    pub works_count: u64,

    /// Citation count
    pub cited_by_count: u64,

    /// ROR ID
    pub ror: Option<String>,

    /// Updated date
    pub updated_date: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_deserialization() {
        let json = r#"{
            "id": "W123",
            "title": "Test Paper",
            "cited_by_count": 10,
            "authorships": [],
            "topics": [],
            "concepts": [],
            "referenced_works": [],
            "related_works": [],
            "grants": []
        }"#;

        let work: Work = serde_json::from_str(json).unwrap();
        assert_eq!(work.id, "W123");
        assert_eq!(work.title, "Test Paper");
        assert_eq!(work.cited_by_count, 10);
    }

    #[test]
    fn test_author_position() {
        let first = serde_json::from_str::<AuthorPosition>(r#""first""#).unwrap();
        assert_eq!(first, AuthorPosition::First);

        let last = serde_json::from_str::<AuthorPosition>(r#""last""#).unwrap();
        assert_eq!(last, AuthorPosition::Last);
    }
}
