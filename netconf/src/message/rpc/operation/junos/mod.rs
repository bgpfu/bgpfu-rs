pub mod open_configuration;
#[doc(inline)]
pub use self::open_configuration::OpenConfiguration;

trivial_ops! {
    /// Close the open configuration database and discard any uncommitted changes.
    ///
    /// See [Juniper documentation][junos-docs].
    ///
    /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-close-configuration.html
    pub operation CloseConfiguration {
        const NAME = "close-configuration";
    }

    /// Request that the NETCONF server open and lock the candidate configuration, enabling the client
    /// application both to read and change it, but preventing any other users or applications from
    /// changing it.
    ///
    /// See [Juniper documentation][junos-docs].
    ///
    /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-lock-configuration.html
    pub operation LockConfiguration {
        const NAME = "lock-configuration";
    }

    /// Request that the NETCONF server unlock and close the candidate configuration.
    ///
    /// See [Juniper documentation][junos-docs].
    ///
    /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-unlock-configuration.html
    pub operation UnlockConfiguration {
        const NAME = "unlock-configuration";
    }
}

// TODO:
// <abort> message
// https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-abort.html
//
// /// Request that the NETCONF or Junos XML protocol server perform one of the variants of the commit
// /// operation on the candidate configuration, a private copy of the candidate configuration, or an
// /// open instance of the ephemeral configuration database.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-commit-configuration.html
// pub operation CommitConfiguration {
//     const NAME = "commit-configuration";
//     type ReplyData = Never;
// }
//
// /// Request checksum information for the specified file.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-get-checksum-information.html
// pub operation GetChecksumInformation {
//     const NAME = "get-checksum-information";
//     type ReplyData = Never;
// }
//
// /// Request configuration data from the NETCONF server.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-get-configuration.html
// pub operation GetConfiguration {
//     const NAME = "get-configuration";
//     type ReplyData = Never;
// }
//
// /// Request that the NETCONF server load configuration data into the candidate configuration or
// /// open configuration database.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-load-configuration.html
// pub operation LoadConfiguration {
//     const NAME = "load-configuration";
//     type ReplyData = Never;
// }
//
// /// Create a private copy of the candidate configuration or open the default instance or a
// /// user-defined instance of the ephemeral configuration database.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-open-configuration.html
// pub operation OpenConfiguration {
//     const NAME = "open-configuration";
//     type ReplyData = Never;
// }
//
// /// Request that the NETCONF server end the current session.
// ///
// /// See [Juniper documentation][junos-docs].
// ///
// /// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-request-end-session.html
// pub operation RequestEndSession {
//     const NAME = "request-end-session";
//     type ReplyData = Never;
// }

macro_rules! trivial_ops {
    ( $(
        $( #[$attr:meta] )*
        $vis:vis operation $oper_ty:ty {
            const NAME = $name:literal;
        }
    )* ) => {
        paste::paste! {
            $(
                #[doc(inline)]
                $vis use self::[<$oper_ty:snake>]::$oper_ty;
                $vis mod [<$oper_ty:snake>] {
                    $( #[$attr] )*
                    #[derive(Debug, Clone, Copy)]
                    $vis struct $oper_ty {
                        _inner: ()
                    }

                    impl $crate::message::rpc::Operation for $oper_ty {
                        const NAME: &'static str = $name;
                        const REQUIRED_CAPABILITIES: $crate::capabilities::Requirements =
                            $crate::capabilities::Requirements::One(
                                $crate::capabilities::Capability::JunosXmlManagementProtocol
                            );
                        type Builder<'a> = Builder<'a>;
                        type ReplyData = $crate::message::rpc::Empty;
                    }

                    impl $crate::message::WriteXml for $oper_ty {
                        fn write_xml<W>(
                            &self,
                            writer: &mut quick_xml::Writer<W>
                        ) -> Result<(), $crate::message::WriteError>
                        where
                            W: std::io::Write,
                        {
                            _ = writer.create_element($name).write_empty()?;
                            Ok(())
                        }
                    }

                    #[derive(Debug, Clone)]
                    #[must_use]
                    #[doc = "Builder for [`" $oper_ty "`] operation request."]
                    $vis struct Builder<'a> {
                        _ctx: &'a $crate::session::Context,
                    }

                    impl<'a> $crate::message::rpc::operation::Builder<'a, $oper_ty> for Builder<'a> {
                        fn new(ctx: &'a $crate::session::Context) -> Self {
                            Self { _ctx: ctx }
                        }
                        fn finish(self) -> Result<$oper_ty, $crate::Error> {
                            Ok($oper_ty { _inner: () })
                        }
                    }
                }
            )*
        }
    };
}
use trivial_ops;
